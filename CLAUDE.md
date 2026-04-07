# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A Tauri 2 + React + Vite desktop launcher for the Chinese-mirror community-server distribution of Apex Legends (R5Reloaded). It is **wire-compatible** with the official R5Reloaded launcher's `RemoteConfig` / `GameManifest` (`checksums.json`) schema, but every URL is configurable so users behind the GFW can point at a Chinese mirror.

Reference repos sit alongside this one and are reading-only:
- `../r5reloaded_launcher` — the official C# WPF launcher we're cloning the protocol from. Look at `launcher/Game/GameFileManager.cs`, `launcher/Services/{ApiService,ReleaseChannelService}.cs`, and `launcher/Core/Models/{RemoteConfig,ReleaseChannel}.cs`, plus `launcher/Game/Models/{GameManifest,ManifestEntry,FileChunk}.cs`.
- `../launcher` — Astro+React+Tailwind+DaisyUI third-party launcher we borrowed visual design from.
- `../mxtools` — Vue+Tauri tool whose Apex launch-options UI inspired ours; `apex启动项大全.txt` at its root is the source of truth for the catalog.

## Commands

Package manager is **pnpm**. The Tauri config wires it into the dev/build hooks.

- `pnpm tauri dev` — Run the full desktop app (spawns Vite + Rust build + native window). This is the normal development command. Mac dev works for everything except `detect/` (Windows-only, returns empty Vec on macOS).
- `pnpm dev` — Frontend only (Vite on port 1420). Use when iterating on React without recompiling Rust.
- `pnpm build` — TypeScript check + Vite production build. Validates the full TS surface.
- `pnpm tauri build` — Distributable desktop bundle.
- `cargo check --manifest-path src-tauri/Cargo.toml` — Fast Rust validation. Run this after every edit to `src-tauri/`.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` — Unit tests live in `launch_options/`, `manifest/filter`, `verify/checksum`, `download/retry`, and `offline/shape_detect`. Run this after backend changes — there's a golden test that locks the default launch-options selection to `["-language", "schinese", "+pylon_matchmaking_hostname", "r5r-org.sleep0.de"]`.

No linter/formatter is configured.

## Architecture

```
React (src/) ── @tauri-apps/api invoke + listen ──> Rust (src-tauri/src/)
                                                    │
                                                    ▼
                          tauri::State<LauncherState>
                          ├─ settings  (parking_lot::RwLock)
                          ├─ http      (HttpClientFactory; rebuilt on proxy change)
                          ├─ jobs      (cancellation tokens for active install jobs)
                          └─ config_dir
```

### Backend (src-tauri/src/)

- `commands/` — IPC thunks only. **Every `#[tauri::command]` lives here.** The actual logic lives in the matching feature module so it's testable without a Tauri runtime. Convention: `commands/foo.rs` calls into `foo/` (e.g. `commands::install` → `download::pipeline` + `offline::*`).
- `state.rs` — `LauncherState` struct (held in `tauri::State`) plus `JobRegistry` for cancelable background jobs.
- `error.rs` — `AppError` enum + serde Serialize impl that produces `{ kind, message }` JSON for the React side. The TS wrapper in `src/ipc/invoke.ts` rethrows it as a `class AppError`.
- `events.rs` — `ProgressEvent` and `LaunchExitedEvent` payloads + the event channel name constants (`install://progress`, `launch://exited`, `proxy://changed`).
- `proxy/` — `ProxyMode` enum (`System | Custom(url) | None`) + `HttpClientFactory` that rebuilds `reqwest::Client` on proxy change. **In-flight requests keep the old client; only new requests use the new one.** System proxy is detected via the `sysproxy` crate, not via env vars (Chinese users rarely set HTTP_PROXY).
- `config/`
  - `remote.rs` — `RemoteConfig` + `Channel` matching the official schema EXACTLY (camelCase JSON via serde rename).
  - `settings.rs` — `LauncherSettings` with `schema_version: u32` + `#[serde(default)]` on every field for forward compat. Persisted as `settings.json` in `app_config_dir()` via atomic write-then-rename.
  - `fetch.rs` — `fetch_remote_config` + `fetch_channel_version`.
  - `paths.rs` — install dir computed as `<library_root>/R5R Library/<CHANNEL_UPPERCASE>/`.
- `detect/` — **Windows-only**, gated with `#[cfg(windows)]`. Three sources, run concurrently:
  - `shortcut.rs` — parses `%ProgramData%\Microsoft\Windows\Start Menu\Programs\R5Reloaded\R5Reloaded.lnk` via the `lnk` crate.
  - `registry.rs` — walks `HKLM\...\Uninstall` (and `WOW6432Node`) via `winreg`, matches `DisplayName` containing "r5reloaded".
  - `library_scan.rs` — probes `C:\Program Files\R5R Library`, `C:\R5R Library`, etc., looking for `<channel>/r5apex.exe`.
  - `stub_unix.rs` — empty Vec on macOS so the maintainer can dev locally.
- `manifest/` — `GameManifest`/`ManifestEntry`/`FileChunk` schema (mirrors official C# types) + `is_user_generated` (skips `platform\cfg\user`, `platform\screenshots`, `platform\logs`) + `is_language_match` + the manifest fetcher.
- `verify/checksum.rs` — async streaming SHA-256 via `tokio::task::spawn_blocking` (don't hog the runtime with disk IO).
- `download/`
  - `pipeline.rs` — `run_install` orchestrator. Takes a `mode: InstallMode { Install | Update | Repair }`. Walks the manifest, builds a plan of mismatched/missing files, executes them in parallel under a global semaphore, then runs a verify pass and persists the version + installed flag.
  - `worker.rs` — single-stream `stream_download` (8 KB chunks, cancellation-checked between chunks, byte-level progress via `ProgressAggregator`).
  - `chunk.rs` — multi-part download path: parallel-fetch up to 8 chunks per file under an inner semaphore, verify each chunk's SHA-256, merge sequentially into the final file, drop the temp dir.
  - `retry.rs` — `RetryPolicy` (15 attempts × 3s backoff for full files, 50 × 3s for chunks; never retry on 404).
  - `progress.rs` — `ProgressAggregator` with 500ms ring buffer for instantaneous speed, 200ms emitter task that fires `install://progress` events.
- `offline/`
  - `shape_detect.rs` — auto-detects whether the user picked a `R5R Library/<channel>/` dir, a `R5R Library/` dir, a single channel folder, or a zip containing any of the above. Rejects ambiguous shapes (multiple channels) with a clear error.
  - `dir_import.rs` / `zip_import.rs` — copy/extract with byte-level progress events.
- `launch_options/` — static `OPTION_CATALOG` built once via `once_cell::Lazy`, hand-authored from `apex启动项大全.txt`. `OptionKind` is `Toggle{args} | Int{flag,min,max} | IntPair{x_flag,y_flag} | Enum{flag,choices} | String{flag,placeholder}`. `compose_launch_args` → `Vec<String>`. `validate_launch_args` → warnings for non-native resolutions + generic `conflicts_with` walk. **Defaults are `-language schinese` + `+pylon_matchmaking_hostname r5r-org.sleep0.de`** — locked in by a unit test.
- `process/launch.rs` — spawns `r5apex.exe` via `tauri-plugin-shell::Command`, captures the PID, drops the `CommandChild` so the launcher closing doesn't kill the game, watches the event stream for `Terminated` and emits `launch://exited`.

Adding a new IPC command: define the `async fn` in the matching feature module, add a thin `#[tauri::command]` thunk in `commands/<feature>.rs`, then list it in the `tauri::generate_handler![...]` macro inside `lib.rs::run()`. Capability allowlist (`src-tauri/capabilities/default.json`) only matters for plugin permissions, not for our own commands.

### Frontend (src/)

- `ipc/` — typed wrappers around `invoke`. `invoke.ts` is the only place that touches `@tauri-apps/api/core` directly; it converts the Rust `AppError` payload into a JS `class AppError`. One TS file per `commands/` group plus `types.ts` mirroring all the Rust DTOs.
- `hooks/`
  - `useSettings.tsx` — global `SettingsContext` with `loadSettings` on mount and `update(patch)` for partial saves.
  - `useInstallProgress.ts` — listens on `install://progress`, returns the latest snapshot. Includes `formatBytes` / `formatEta` helpers.
  - `useLaunchExited.ts` — listens on `launch://exited`.
- `components/` — small set of presentational primitives: `Sidebar`, `GlassCard` + `SectionHeader`, `PrimaryButton`, `InstallProgress`.
- `pages/` — one file per top-level tab: `HomeTab`, `LaunchOptionsTab`, `SettingsTab`, `AboutTab`.
- Styling is Tailwind 4 (zero config — `@import "tailwindcss"` in `src/index.css`) + glass-morphism CSS variables defined in `src/index.css`. Dark theme only. **DO NOT** add DaisyUI or any other component lib.

### Capability allowlist gotcha

`src-tauri/capabilities/default.json` must list every plugin permission. We use `core:default`, `opener:default`, `store:default`, `dialog:default`, `fs:default`, `shell:default`, `shell:allow-open`. Adding a new Tauri 2 plugin without updating this file silently no-ops the plugin's commands — easy to lose an hour to.

## Testing strategy

- **Unit tests** (`cargo test --lib`) cover: `compose_launch_args` golden output, `validate_launch_args` non-native resolution warning, conflict detection, `is_user_generated`, `is_language_match`, SHA-256 against a known input, `RetryPolicy` (success after retries + no retry on 404), `offline::shape_detect` for all four ambiguous-shape cases.
- **Manual smoke test on Mac** (no game, no real mirror): `pnpm tauri dev`, set proxy to None, type a Chinese path into the install picker (validator should reject), open Launch Options tab and verify defaults compose correctly in the live preview.
- **Manual smoke test on Windows** (real game): place an `R5Reloaded.lnk` somewhere, click "检测", verify the path appears. Stand up a tiny local HTTP server with a hand-crafted `config.json` + `version.txt` + `checksums.json` + a few small files to validate the install pipeline end-to-end. The chunked-merge logic is the highest-risk piece.

## Open known risks

- **`tauri-plugin-shell` Command detachment** — we drop the `CommandChild` after capturing the PID and trust that the game survives the launcher closing. Untested on real Windows because the dev environment is Mac. Fallback: switch `process/launch.rs` to raw `std::process::Command::spawn` + `sysinfo` polling.
- **`channel-key` header leakage** — only requests built via `download::worker::stream_download` and `manifest::fetch::fetch_manifest` / `config::fetch::fetch_channel_version` attach the channel key, and only when `channel.requires_key`. Don't add the key to the root config fetch — it goes to a different host.
- **Language file handling** — v1 hardcodes `["schinese"]` in `download::pipeline::run_install` (search for `languages_wanted`). The catalog supports more languages forward-compat-wise, but the UI doesn't expose a language selector.
- **Concurrent downloads** — `settings.concurrent_downloads` is the file-level cap (default 4). The chunk-level cap is hardcoded at 8 in `download/chunk.rs::MAX_PARTS_PER_FILE`.
