use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::offline::shape_detect::{zip_entry_keep, DetectedZipShape};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// Stream-extract a zip into `install_root`, keeping only the top-level
/// `R5R Library/` and `R5R Launcher/` folders (everything else in the pack —
/// README files, vendor scripts — is silently skipped). Paths are preserved
/// verbatim so the content lands at `<install_root>/R5R Library/...` and
/// `<install_root>/R5R Launcher/...`, matching the overlay-onto-R5Reloaded
/// layout the official packs document.
///
/// Also used by the patch update flow, which gets a zip of the same shape —
/// for patches `install_root` points at the user's `library_root`, and the
/// `File::create` overwrite semantics directly replace the outdated files.
pub async fn import_zip(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    shape: &DetectedZipShape,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<()> {
    extract_keep_prefixes(
        app,
        job_id,
        zip_path,
        install_root,
        shape.total_bytes,
        shape.file_count,
        cancel,
    )
    .await?;
    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.into(), InstallPhase::Complete),
    );
    Ok(())
}

/// Core zip extraction. Iterates the zip's central directory, extracting every
/// regular-file entry under `R5R Library/` or `R5R Launcher/` to
/// `install_root/<entry path>`, overwriting existing files in place. Emits
/// `install://progress` events at ~200 ms cadence.
///
/// `total_bytes` and `file_count` should come from a prior `detect_zip` call
/// so we avoid a second central-directory scan. When `total_bytes` is zero
/// (data-descriptor zip), the UI falls back to file-count progress.
///
/// Does NOT emit `Complete` — the caller decides whether this extraction is
/// the last step of the job.
pub async fn extract_keep_prefixes(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    install_root: &Path,
    total_bytes: u64,
    file_count: usize,
    cancel: CancellationToken,
) -> AppResult<()> {
    std::fs::create_dir_all(install_root)?;

    // Emit Downloading immediately so the UI flips past Preparing the moment
    // extraction starts.
    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent {
            job_id: job_id.into(),
            phase: InstallPhase::Downloading,
            file_index: 0,
            file_count,
            bytes_done: 0,
            bytes_total: total_bytes,
            current_file: String::new(),
            speed_bps: 0,
            eta_seconds: 0,
        },
    );

    let f = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::other(format!("无法打开 zip: {}", e)))?;

    let mut bytes_done: u64 = 0;
    let mut file_index: usize = 0;
    let started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    for i in 0..archive.len() {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::other(format!("zip 条目: {}", e)))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if !zip_entry_keep(&name) {
            continue;
        }
        // Zip entries may use `/` and we want native separators on disk. Also
        // guard against absolute / `..` components — the zip crate does this
        // via `enclosed_name()`, which returns None for unsafe paths.
        let rel = match entry.enclosed_name() {
            Some(p) => p,
            None => continue,
        };
        let rel_buf = PathBuf::from(rel);
        if rel_buf.as_os_str().is_empty() {
            continue;
        }
        let dst = install_root.join(&rel_buf);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&dst)?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let n = entry.read(&mut buf)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            bytes_done += n as u64;

            if last_emit.elapsed().as_millis() > 200 {
                last_emit = std::time::Instant::now();
                let speed = if started.elapsed().as_secs_f64() > 0.0 {
                    (bytes_done as f64 / started.elapsed().as_secs_f64()) as u64
                } else {
                    0
                };
                let eta = if speed > 0 {
                    total_bytes.saturating_sub(bytes_done) / speed
                } else {
                    0
                };
                let _ = app.emit(
                    EVT_INSTALL_PROGRESS,
                    ProgressEvent {
                        job_id: job_id.into(),
                        phase: InstallPhase::Downloading,
                        file_index,
                        file_count,
                        bytes_done,
                        bytes_total: total_bytes,
                        current_file: rel_buf.display().to_string(),
                        speed_bps: speed,
                        eta_seconds: eta,
                    },
                );
            }
        }
        out.sync_all()?;
        file_index += 1;
    }

    Ok(())
}
