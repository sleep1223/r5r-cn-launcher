//! Patch update path: download a zip from `dashboard.patches[]` and extract
//! it on top of the install. The patch zip has the same layout as the offline
//! pack (top-level `R5R Library/` + optional `R5R Launcher/`), so extraction
//! into `library_root` directly overwrites the outdated files.
//!
//! Scope is intentionally narrow: only a single-hop patch (local version must
//! equal some entry's `from_version` and that entry's `to_version` must match
//! the remote version). Multi-hop chaining falls back to the full verify
//! pipeline in `pipeline.rs`.

use crate::dashboard::{fetch_dashboard_config, PatchEntry};
use crate::download::worker::entry_local_path;
use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::manifest::{is_language_match, is_user_generated, GameManifest};
use crate::offline::shape_detect::scan_zip_kept_entries;
use crate::offline::zip_import::extract_keep_prefixes;
use crate::state::LauncherState;
use crate::verify::sha256_file;
use futures::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

/// Result of an applied patch. `NotApplicable` is the explicit signal that
/// `pipeline::run_install` should fall back to the verify path.
pub enum PatchOutcome {
    Applied,
    NotApplicable,
}

pub async fn apply_patch(
    app: &AppHandle,
    state: &LauncherState,
    job_id: &str,
    channel_name: &str,
    from_version: &str,
    to_version: &str,
    target_manifest: &GameManifest,
    cancel: CancellationToken,
) -> AppResult<PatchOutcome> {
    if from_version.is_empty() || to_version.is_empty() || from_version == to_version {
        return Ok(PatchOutcome::NotApplicable);
    }

    // 1. Look up an applicable patch entry.
    let (dashboard_url, library_root, download_hd_textures) = {
        let s = state.settings.read();
        (
            s.dashboard_api_url.clone(),
            s.library_root.clone(),
            s.download_hd_textures,
        )
    };
    if library_root.is_empty() {
        return Err(AppError::settings("尚未配置安装根目录"));
    }
    let client: Client = state.http.read().await.client();

    emit_downloading(app, job_id, 0, 0, 0, "拉取补丁元数据 …");
    let dashboard = fetch_dashboard_config(&client, &dashboard_url).await?;
    let patch = match find_patch(&dashboard.patches, from_version, to_version) {
        Some(p) => p.clone(),
        None => return Ok(PatchOutcome::NotApplicable),
    };
    if patch.url.is_empty() {
        return Ok(PatchOutcome::NotApplicable);
    }

    // 2. Download the patch zip to a temp file under the install root.
    let lib_path = PathBuf::from(&library_root);
    let tmp_dir = lib_path.join(".cn-launcher-tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let tmp_zip = tmp_dir.join(format!("patch-{}-to-{}.zip", from_version, to_version));

    download_to_file(app, job_id, &client, &patch.url, &tmp_zip, &cancel).await?;
    validate_downloaded_patch(&tmp_zip, &patch).await?;

    // 3. Extract on top of `library_root` — overwrites in place.
    let (total_bytes, file_count) = scan_zip_kept_entries(&tmp_zip)?;
    if file_count == 0 {
        // Zip has nothing under R5R Library/ or R5R Launcher/ — treat as
        // not-applicable so we fall back to verify rather than silently
        // declare success.
        let _ = std::fs::remove_file(&tmp_zip);
        return Ok(PatchOutcome::NotApplicable);
    }
    extract_keep_prefixes(
        app,
        job_id,
        &tmp_zip,
        &lib_path,
        channel_name,
        total_bytes,
        file_count,
        cancel.clone(),
    )
    .await?;
    let _ = std::fs::remove_file(&tmp_zip);

    // 4. Verify the patched install against the target manifest before
    //    accepting the version bump. If this fails the caller falls back to
    //    the normal verify/download pipeline, which repairs the partial state.
    emit_downloading(app, job_id, 0, 0, 0, "校验补丁包结果 …");
    verify_patched_install(
        &lib_path,
        channel_name,
        target_manifest,
        download_hd_textures,
        cancel.clone(),
    )
    .await?;

    // 5. Bump version.
    {
        let mut s = state.settings.write();
        let entry = s.channels.entry(channel_name.to_string()).or_default();
        entry.version = to_version.to_string();
        entry.installed = true;
    }
    let _ = state.save_settings();

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.into(), InstallPhase::Complete),
    );
    Ok(PatchOutcome::Applied)
}

async fn verify_patched_install(
    library_root: &Path,
    channel_name: &str,
    manifest: &GameManifest,
    download_hd_textures: bool,
    cancel: CancellationToken,
) -> AppResult<()> {
    let install_dir = library_root
        .join("R5R Library")
        .join(channel_name.to_uppercase());
    let languages_wanted = ["schinese"];

    for entry in manifest.files.iter().filter(|entry| {
        !is_user_generated(&entry.path)
            && if entry.optional {
                download_hd_textures && entry.language.is_empty()
            } else if entry.language.is_empty() {
                true
            } else {
                is_language_match(entry, &languages_wanted)
            }
    }) {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        if entry.checksum.is_empty() {
            continue;
        }
        let local = entry_local_path(&install_dir, &entry.path);
        if !local.exists() {
            return Err(AppError::Verification {
                path: entry.path.clone(),
                expected: entry.checksum.clone(),
                actual: "missing".to_string(),
            });
        }
        let actual = sha256_file(&local).await?;
        if !actual.eq_ignore_ascii_case(&entry.checksum) {
            return Err(AppError::Verification {
                path: entry.path.clone(),
                expected: entry.checksum.clone(),
                actual,
            });
        }
    }

    Ok(())
}

fn find_patch<'a>(
    patches: &'a [PatchEntry],
    from_version: &str,
    to_version: &str,
) -> Option<&'a PatchEntry> {
    patches
        .iter()
        .find(|p| p.from_version == from_version && p.to_version == to_version && !p.url.is_empty())
}

async fn validate_downloaded_patch(path: &Path, patch: &PatchEntry) -> AppResult<()> {
    if patch.size > 0 {
        let actual = tokio::fs::metadata(path).await?.len();
        if actual != patch.size {
            return Err(AppError::Verification {
                path: patch.url.clone(),
                expected: patch.size.to_string(),
                actual: actual.to_string(),
            });
        }
    }

    if !patch.checksum.is_empty() {
        let actual = sha256_file(path).await?;
        if !actual.eq_ignore_ascii_case(&patch.checksum) {
            return Err(AppError::Verification {
                path: patch.url.clone(),
                expected: patch.checksum.clone(),
                actual,
            });
        }
    }

    Ok(())
}

async fn download_to_file(
    app: &AppHandle,
    job_id: &str,
    client: &Client,
    url: &str,
    dest: &Path,
    cancel: &CancellationToken,
) -> AppResult<()> {
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    let resp = tokio::time::timeout(Duration::from_secs(15), client.get(url).send())
        .await
        .map_err(|_| AppError::http(format!("GET {}: 服务器无响应 (15s)", url)))?
        .map_err(|e| AppError::http(format!("GET {}: {}", url, e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "{} HTTP {}",
            url,
            resp.status().as_u16()
        )));
    }
    let total = resp.content_length().unwrap_or(0);

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();
    let mut bytes_done: u64 = 0;
    let started = Instant::now();
    let mut last_emit = Instant::now();

    while let Some(item) = stream.next().await {
        if cancel.is_cancelled() {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(AppError::Cancelled);
        }
        let chunk = item.map_err(|e| AppError::http(format!("read body: {}", e)))?;
        file.write_all(&chunk).await?;
        bytes_done += chunk.len() as u64;

        if last_emit.elapsed().as_millis() > 200 {
            last_emit = Instant::now();
            let speed = if started.elapsed().as_secs_f64() > 0.0 {
                (bytes_done as f64 / started.elapsed().as_secs_f64()) as u64
            } else {
                0
            };
            emit_downloading(app, job_id, bytes_done, total, speed, "下载补丁包中 …");
        }
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok(())
}

fn emit_downloading(
    app: &AppHandle,
    job_id: &str,
    bytes_done: u64,
    bytes_total: u64,
    speed_bps: u64,
    current_file: &str,
) {
    let eta = if speed_bps > 0 {
        bytes_total.saturating_sub(bytes_done) / speed_bps
    } else {
        0
    };
    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent {
            job_id: job_id.into(),
            phase: InstallPhase::Downloading,
            file_index: 0,
            file_count: 0,
            bytes_done,
            bytes_total,
            current_file: current_file.into(),
            speed_bps,
            eta_seconds: eta,
        },
    );
}
