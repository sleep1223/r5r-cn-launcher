# Repository Guidelines

## Project Structure & Module Organization

This repository is a Tauri 2 desktop launcher with a React/Vite frontend and a Rust backend.

- `src/` contains the frontend: `components/`, `pages/`, `hooks/`, `ipc/`, `api/`, and shared utilities.
- `src-tauri/src/` contains backend logic. Keep `#[tauri::command]` handlers in `commands/`; put testable business logic in matching feature modules such as `download/`, `offline/`, `launch_options/`, `manifest/`, `proxy/`, and `process/`.
- `src-tauri/icons/`, `src-tauri/keys/`, `src-tauri/capabilities/`, and `public/` hold desktop assets, updater keys, permissions, and static web assets.
- Unit tests currently live inline in Rust modules under `#[cfg(test)]`.

## Build, Test, and Development Commands

Use `pnpm`; the Tauri config calls pnpm from its dev/build hooks.

- `pnpm install` installs frontend and Tauri CLI dependencies.
- `pnpm tauri dev` runs the full desktop app with Vite, Rust compilation, and a native window.
- `pnpm dev` runs the frontend only, useful for React UI iteration.
- `pnpm build` runs `tsc` and creates the Vite production build.
- `pnpm tauri build` creates the distributable desktop bundle.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` checks Rust formatting without modifying files.
- `cargo check --manifest-path src-tauri/Cargo.toml` validates Rust backend changes quickly.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib` runs backend unit tests.

After any file change, run the default gates before handing off: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo check --manifest-path src-tauri/Cargo.toml`. Version bumps use the release-specific gates below instead of these default gates. Do not run full compile/build or test commands unless the user specifically asks for them.

## Coding Style & Naming Conventions

TypeScript is strict (`noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`) and uses React JSX. Prefer `PascalCase` for components/pages, `useXxx` for hooks, and small typed IPC wrappers in `src/ipc/`.

Rust uses the standard module naming style: `snake_case` files/modules, `PascalCase` types, and `snake_case` functions. No project-specific formatter or linter is configured; use standard `cargo fmt` checks for Rust formatting. There is currently no ESLint, Prettier, Clippy, or frontend test script.

## Testing Guidelines

Add Rust unit tests beside the code they cover. Existing tests focus on launch-option composition/validation, manifest filtering, checksum verification, retry policy, updater helpers, and offline import shape detection. After backend edits, default to `cargo check --manifest-path src-tauri/Cargo.toml`; run `cargo test --manifest-path src-tauri/Cargo.toml --lib`, `pnpm build`, or other full build/test gates only when explicitly requested. `pnpm build` is the frontend validation gate because it runs TypeScript checking via `tsc` before Vite builds.

## Commit & Pull Request Guidelines

Recent commits use release-style subjects such as `v0.36.0: launch EA App and wait for its process before launching the game`. Keep subjects concise, imperative or descriptive, and scoped to one behavior change. PRs should include a short summary, validation commands run, linked issue when applicable, and screenshots or recordings for UI changes.

## Release & Configuration Notes

Do not bump versions unless explicitly requested. A release bump must update `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.lock` together so local version files match the release tag.

For a version bump, run only these two local validation commands before committing or pushing:

- `cargo check --manifest-path src-tauri/Cargo.toml --lib --locked`
- `pnpm exec tsc --noEmit --incremental false`

Do not run local compile, build, bundle, or test commands as part of a version bump. In particular, do not run `cargo test`, `cargo build`, `pnpm build`, `pnpm tauri dev`, or `pnpm tauri build`. The two check-only commands above are the complete local release gate. Both commands must pass before committing, tagging, or pushing anything to GitHub.

After both checks pass, commit the version change, create the matching `vX.Y.Z` tag, and push both the current branch and the tag to the remote GitHub repository so the release workflow is triggered. The release workflow installs with `pnpm install --frozen-lockfile` and builds Windows artifacts with `pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis`. Never commit signing private keys; release signing uses GitHub Actions secrets.
