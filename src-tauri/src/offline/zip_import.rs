use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::offline::shape_detect::DetectedZipShape;
use std::io::{Read, Write};
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// Stream-extract a zip into `<install_root>/R5R Library/<channel>/`.
/// Entries that don't start with `shape.strip_prefix` are silently skipped
/// (so a zip that ships extras like a README at the root won't break the import).
pub async fn import_zip(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    shape: &DetectedZipShape,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<()> {
    let dest_root = install_root.join("R5R Library").join(&shape.channel);
    std::fs::create_dir_all(&dest_root)?;

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.into(), InstallPhase::Preparing),
    );

    // Open archive twice — once to compute totals, once to extract. Zip
    // uncompressed sizes are in the local file headers so this is cheap.
    let f = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::other(format!("无法打开 zip: {}", e)))?;

    let mut total_bytes: u64 = 0;
    let mut file_count: usize = 0;
    for i in 0..archive.len() {
        let e = archive
            .by_index(i)
            .map_err(|e| AppError::other(format!("zip 条目: {}", e)))?;
        if e.is_dir() {
            continue;
        }
        if !e.name().starts_with(&shape.strip_prefix) {
            continue;
        }
        total_bytes += e.size();
        file_count += 1;
    }

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
        if !name.starts_with(&shape.strip_prefix) {
            continue;
        }
        let rel = &name[shape.strip_prefix.len()..];
        if rel.is_empty() {
            continue;
        }
        let dst = dest_root.join(rel);
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
                        current_file: rel.to_string(),
                        speed_bps: speed,
                        eta_seconds: eta,
                    },
                );
            }
        }
        out.sync_all()?;
        file_index += 1;
    }

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.into(), InstallPhase::Complete),
    );
    Ok(())
}
