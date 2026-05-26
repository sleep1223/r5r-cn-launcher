use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::offline::shape_detect::DetectedShape;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

/// Copy the contents of `shape.source_root` into
/// `<install_root>/R5R Library/<shape.channel>/`.
///
/// Emits `install://progress` events with byte-level granularity.
pub async fn import_directory(
    app: &AppHandle,
    job_id: &str,
    shape: &DetectedShape,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<()> {
    let dest_root = install_root.join("R5R Library").join(&shape.channel);
    std::fs::create_dir_all(&dest_root)?;

    // Plan: walk the source tree once, collect all regular files + total size.
    // Emit Preparing progress periodically and check cancel between entries so
    // the 取消 button actually interrupts a slow scan on spinning disks.
    emit(
        app,
        ProgressEvent::empty(job_id.into(), InstallPhase::Preparing),
    );
    let mut files: Vec<(PathBuf, PathBuf, u64)> = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut scanned: usize = 0;
    let mut last_scan_emit = Instant::now();
    for entry in WalkDir::new(&shape.source_root) {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        let entry = entry.map_err(|e| AppError::other(format!("walkdir: {}", e)))?;
        scanned += 1;
        if !entry.file_type().is_file() {
            if last_scan_emit.elapsed().as_millis() > 200 {
                last_scan_emit = Instant::now();
                emit(
                    app,
                    ProgressEvent {
                        job_id: job_id.into(),
                        phase: InstallPhase::Preparing,
                        file_index: scanned,
                        file_count: 0,
                        bytes_done: 0,
                        bytes_total: 0,
                        current_file: String::new(),
                        speed_bps: 0,
                        eta_seconds: 0,
                    },
                );
            }
            continue;
        }
        let src = entry.into_path();
        let rel = src
            .strip_prefix(&shape.source_root)
            .map_err(|e| AppError::other(e.to_string()))?
            .to_path_buf();
        let dst = dest_root.join(&rel);
        let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
        total_bytes += size;
        files.push((src, dst, size));
        if last_scan_emit.elapsed().as_millis() > 200 {
            last_scan_emit = Instant::now();
            emit(
                app,
                ProgressEvent {
                    job_id: job_id.into(),
                    phase: InstallPhase::Preparing,
                    file_index: scanned,
                    file_count: 0,
                    bytes_done: 0,
                    bytes_total: 0,
                    current_file: String::new(),
                    speed_bps: 0,
                    eta_seconds: 0,
                },
            );
        }
    }
    let file_count = files.len();

    let bytes_done = Arc::new(AtomicU64::new(0));
    let started = Instant::now();

    let app_emit = app.clone();
    let job_id_owned = job_id.to_string();
    let bytes_done_emit = bytes_done.clone();
    let cancel_emit = cancel.clone();
    // Periodic emitter task — fires every 200ms with the latest snapshot.
    let emitter = tauri::async_runtime::spawn(async move {
        let mut t = tokio::time::interval(std::time::Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = cancel_emit.cancelled() => break,
                _ = t.tick() => {
                    let done = bytes_done_emit.load(Ordering::Relaxed);
                    let speed = if started.elapsed().as_secs_f64() > 0.0 {
                        (done as f64 / started.elapsed().as_secs_f64()) as u64
                    } else { 0 };
                    let eta = if speed > 0 { total_bytes.saturating_sub(done) / speed } else { 0 };
                    let _ = app_emit.emit(EVT_INSTALL_PROGRESS, ProgressEvent {
                        job_id: job_id_owned.clone(),
                        phase: InstallPhase::Downloading, // re-using "Downloading" for offline copy
                        file_index: 0,
                        file_count,
                        bytes_done: done,
                        bytes_total: total_bytes,
                        current_file: String::new(),
                        speed_bps: speed,
                        eta_seconds: eta,
                    });
                    if done >= total_bytes && total_bytes > 0 { break; }
                }
            }
        }
    });

    // Copy each file.
    for (i, (src, dst, _size)) in files.iter().enumerate() {
        if cancel.is_cancelled() {
            emitter.abort();
            return Err(AppError::Cancelled);
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        copy_file_with_progress(src, dst, &bytes_done)?;

        // Lightweight per-file event so the UI can show "current file".
        let _ = app.emit(
            EVT_INSTALL_PROGRESS,
            ProgressEvent {
                job_id: job_id.into(),
                phase: InstallPhase::Downloading,
                file_index: i + 1,
                file_count,
                bytes_done: bytes_done.load(Ordering::Relaxed),
                bytes_total: total_bytes,
                current_file: dst
                    .strip_prefix(install_root)
                    .map(crate::util::display_slash)
                    .unwrap_or_default(),
                speed_bps: 0,
                eta_seconds: 0,
            },
        );
    }

    emitter.abort();
    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.into(), InstallPhase::Complete),
    );
    Ok(())
}

fn emit(app: &AppHandle, ev: ProgressEvent) {
    let _ = app.emit(EVT_INSTALL_PROGRESS, ev);
}

fn copy_file_with_progress(src: &Path, dst: &Path, counter: &Arc<AtomicU64>) -> AppResult<()> {
    use std::io::{Read, Write};
    let mut input = std::fs::File::open(src)?;
    let mut output = std::fs::File::create(dst)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        output.write_all(&buf[..n])?;
        counter.fetch_add(n as u64, Ordering::Relaxed);
    }
    output.sync_all()?;
    Ok(())
}
