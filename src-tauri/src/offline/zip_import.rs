use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::offline::shape_detect::{preflight_zip, zip_entry_keep};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// Streaming-extract an offline pack zip without ever touching the central
/// directory or EOCD. Reads local file headers sequentially via
/// `zip::read::read_zipfile_from_stream` and writes every entry under
/// `R5R Library/` or `R5R Launcher/` to `install_root/<entry path>`.
///
/// This is the path used by the offline-import flow: it tolerates zips whose
/// EOCD is missing or unreadable (truncated downloads, trailing junk, some
/// Zip64 producers) — the common failure mode for the 25+ GB packs users
/// download from mirror sites. Preflight still runs to reject 7z/RAR/split
/// archives up-front, since those can't be streamed either.
///
/// Returns the channel name inferred from the first
/// `R5R Library/<channel>/r5apex.exe` entry seen. Errors if the stream ends
/// without one — we can't commit settings to an invalid pack.
///
/// Progress events have `bytes_total=0` and `file_count=0` (we don't know the
/// totals without the central directory); the UI falls back to a running
/// bytes/file counter plus the current entry name.
pub async fn import_zip_streaming(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<String> {
    preflight_zip(zip_path)?;

    let app_clone = app.clone();
    let jid_worker = job_id.to_string();
    let zp = zip_path.to_path_buf();
    let ir = install_root.to_path_buf();

    let channel = tokio::task::spawn_blocking(move || {
        extract_streaming_blocking(&app_clone, &jid_worker, &zp, &ir, cancel)
    })
    .await
    .map_err(|e| AppError::other(format!("zip 解压任务中断: {}", e)))??;

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.to_string().into(), InstallPhase::Complete),
    );
    Ok(channel)
}

fn extract_streaming_blocking(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<String> {
    std::fs::create_dir_all(install_root)?;

    let f = std::fs::File::open(zip_path)?;
    // 1 MiB BufReader keeps the sequential-read syscall rate down on spinning
    // disks without bloating memory — the local header parse itself only needs
    // a few hundred bytes at a time.
    let mut reader = BufReader::with_capacity(1024 * 1024, f);

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent {
            job_id: job_id.into(),
            phase: InstallPhase::Downloading,
            file_index: 0,
            file_count: 0,
            bytes_done: 0,
            bytes_total: 0,
            current_file: String::new(),
            speed_bps: 0,
            eta_seconds: 0,
        },
    );

    let mut bytes_done: u64 = 0;
    let mut file_index: usize = 0;
    let mut channel: Option<String> = None;
    let started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    loop {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }

        let maybe = zip::read::read_zipfile_from_stream(&mut reader)
            .map_err(|e| AppError::other(format!("读取 zip 流失败: {}", e)))?;
        let mut entry = match maybe {
            Some(e) => e,
            None => break, // hit the central directory — we're done.
        };

        if entry.is_dir() {
            continue; // drop drains remaining bytes automatically.
        }
        let name = entry.name().to_string();
        if !zip_entry_keep(&name) {
            continue;
        }
        let rel = match entry.enclosed_name() {
            Some(p) => p,
            None => continue,
        };
        let rel_buf = PathBuf::from(rel);
        if rel_buf.as_os_str().is_empty() {
            continue;
        }

        if channel.is_none() {
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("r5r library/") && lower.ends_with("/r5apex.exe") {
                let parts: Vec<&str> = name.split('/').collect();
                if parts.len() >= 3 {
                    channel = Some(parts[parts.len() - 2].to_string());
                }
            }
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
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let speed = (bytes_done as f64 / elapsed) as u64;
                let _ = app.emit(
                    EVT_INSTALL_PROGRESS,
                    ProgressEvent {
                        job_id: job_id.into(),
                        phase: InstallPhase::Downloading,
                        file_index,
                        file_count: 0,
                        bytes_done,
                        bytes_total: 0,
                        current_file: rel_buf.display().to_string(),
                        speed_bps: speed,
                        eta_seconds: 0,
                    },
                );
            }
        }
        out.sync_all()?;
        file_index += 1;
    }

    channel.ok_or_else(|| {
        AppError::InvalidPath(
            "zip 已读到末尾但未发现 R5R Library/<频道>/r5apex.exe。\
             请确认这是一个有效的 R5R 离线包；\
             已落盘的文件可能不完整，建议手动清理后重试。"
                .into(),
        )
    })
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
