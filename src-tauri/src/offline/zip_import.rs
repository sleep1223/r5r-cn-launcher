use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::offline::shape_detect::{friendly_zip_open_err, preflight_zip, zip_entry_keep};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// What a prior central-directory walk found — enough to drive extraction
/// without a second scan.
#[derive(Debug, Clone)]
pub struct ExtractionPlan {
    /// Channel name inferred from `R5R Library/<channel>/r5apex.exe`.
    pub channel: String,
    /// Indices into the original `ZipArchive` we intend to extract, in file
    /// order (same order as the central directory).
    pub kept_indices: Vec<usize>,
    /// Sum of `compressed_size()` over kept entries — denominator of the
    /// compressed-byte progress. Non-kept entries are never decompressed, so
    /// they stay out of the total; otherwise the bar could never reach 100%.
    pub kept_compressed_total: u64,
}

/// Walk the central directory once: validate the anchor, enumerate kept
/// entries, and sum their compressed sizes. Any EOCD/CD failure surfaces as
/// the `friendly_zip_open_err` three-reason Chinese message — no streaming
/// fallback, no partial-write side-effects.
pub fn plan_extraction(zip_path: &Path) -> AppResult<ExtractionPlan> {
    let f = std::fs::File::open(zip_path)?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut archive = zip::ZipArchive::new(f).map_err(|e| friendly_zip_open_err(size, &e))?;

    let mut channel: Option<String> = None;
    let mut kept_indices: Vec<usize> = Vec::new();
    let mut kept_compressed_total: u64 = 0;

    for i in 0..archive.len() {
        let e = archive
            .by_index(i)
            .map_err(|err| AppError::other(format!("zip 条目 #{} 读取失败: {}", i, err)))?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        if !zip_entry_keep(&name) {
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

        kept_indices.push(i);
        kept_compressed_total = kept_compressed_total.saturating_add(e.compressed_size());
    }

    let channel = channel.ok_or_else(|| {
        AppError::InvalidPath(
            "未在 zip 中发现 R5R Library/<频道>/r5apex.exe。\
             请确认这是一个完整有效的 R5R 离线包。"
                .into(),
        )
    })?;

    Ok(ExtractionPlan {
        channel,
        kept_indices,
        kept_compressed_total,
    })
}

/// Strict offline-zip import. Requires a readable EOCD / central directory;
/// surfaces `friendly_zip_open_err` otherwise. Emits `install://progress`
/// events with compressed-byte progress + ETA so the UI's existing bar and
/// time-remaining display work unchanged.
///
/// The anchor (`R5R Library/<channel>/r5apex.exe`) is checked before any
/// bytes hit disk — invalid packs fail fast without a half-written library.
pub async fn import_zip_strict(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<String> {
    preflight_zip(zip_path)?;
    let plan = plan_extraction(zip_path)?;
    let channel = plan.channel.clone();

    let app_clone = app.clone();
    let jid = job_id.to_string();
    let zp = zip_path.to_path_buf();
    let ir = install_root.to_path_buf();

    tokio::task::spawn_blocking(move || extract_with_plan(&app_clone, &jid, &zp, &ir, plan, cancel))
        .await
        .map_err(|e| AppError::other(format!("zip 解压任务中断: {}", e)))??;

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.to_string(), InstallPhase::Complete),
    );
    Ok(channel)
}

fn extract_with_plan(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    install_root: &Path,
    plan: ExtractionPlan,
    cancel: CancellationToken,
) -> AppResult<()> {
    std::fs::create_dir_all(install_root)?;

    let f = std::fs::File::open(zip_path)?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut archive = zip::ZipArchive::new(f).map_err(|e| friendly_zip_open_err(size, &e))?;

    let file_count = plan.kept_indices.len();
    let total = plan.kept_compressed_total;

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent {
            job_id: job_id.into(),
            phase: InstallPhase::Downloading,
            file_index: 0,
            file_count,
            bytes_done: 0,
            bytes_total: total,
            current_file: String::new(),
            speed_bps: 0,
            eta_seconds: 0,
        },
    );

    let mut compressed_done_completed: u64 = 0;
    let mut file_index: usize = 0;
    let started = std::time::Instant::now();
    let mut last_emit = std::time::Instant::now();

    for &idx in &plan.kept_indices {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }

        let mut entry = archive
            .by_index(idx)
            .map_err(|e| AppError::other(format!("zip 条目 #{} 读取失败: {}", idx, e)))?;

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

        let entry_compressed = entry.compressed_size();
        let entry_decompressed = entry.size();
        let mut entry_decompressed_read: u64 = 0;

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
            entry_decompressed_read += n as u64;

            if last_emit.elapsed().as_millis() > 200 {
                last_emit = std::time::Instant::now();
                // Interpolate compressed progress within the current entry.
                // One deflate stream has a constant ratio, so decompressed
                // progress maps near-linearly to compressed progress — this
                // kills the "flat line then jump" pattern that per-file
                // granularity produces on multi-GB VPKs.
                let in_entry_compressed = if entry_decompressed == 0 {
                    entry_compressed
                } else {
                    ((entry_compressed as u128 * entry_decompressed_read as u128)
                        / entry_decompressed as u128) as u64
                };
                let compressed_done =
                    compressed_done_completed.saturating_add(in_entry_compressed);
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let speed = (compressed_done as f64 / elapsed) as u64;
                let eta = if speed > 0 {
                    total.saturating_sub(compressed_done) / speed
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
                        bytes_done: compressed_done,
                        bytes_total: total,
                        current_file: crate::util::display_slash(&rel_buf),
                        speed_bps: speed,
                        eta_seconds: eta,
                    },
                );
            }
        }

        out.sync_all()?;
        compressed_done_completed =
            compressed_done_completed.saturating_add(entry_compressed);
        file_index += 1;
    }

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
                        current_file: crate::util::display_slash(&rel_buf),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_zip<F: FnOnce(&mut ZipWriter<std::fs::File>)>(path: &Path, build: F) {
        let f = std::fs::File::create(path).unwrap();
        let mut zw = ZipWriter::new(f);
        build(&mut zw);
        zw.finish().unwrap();
    }

    #[test]
    fn plan_rejects_broken_eocd_with_friendly_message() {
        let td = tempdir().unwrap();
        let p = td.path().join("broken.zip");
        // Valid local-header magic + padding, but no central directory / EOCD
        // anywhere. Preflight passes (signature is fine), ZipArchive::new
        // fails — exactly the scenario we want to reject now.
        let mut buf = vec![0x50, 0x4b, 0x03, 0x04];
        buf.extend(std::iter::repeat(0u8).take(512));
        std::fs::write(&p, buf).unwrap();

        let err = plan_extraction(&p).unwrap_err();
        assert!(matches!(err, AppError::InvalidPath(_)), "got {:?}", err);
        let msg = err.to_string();
        assert!(msg.contains("EOCD"), "got {}", msg);
    }

    #[test]
    fn plan_rejects_pack_missing_r5apex_anchor() {
        let td = tempdir().unwrap();
        let p = td.path().join("no_anchor.zip");
        write_zip(&p, |zw| {
            zw.start_file("R5R Library/LIVE/readme.txt", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"hello").unwrap();
        });

        let err = plan_extraction(&p).unwrap_err();
        assert!(err.to_string().contains("r5apex.exe"), "got {}", err);
    }

    #[test]
    fn plan_finds_channel_and_sums_compressed_total() {
        let td = tempdir().unwrap();
        let p = td.path().join("ok.zip");
        write_zip(&p, |zw| {
            // Non-kept entry — must not contribute to totals.
            zw.start_file("README.md", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"top-level readme").unwrap();

            // Kept anchor — drives channel detection.
            zw.start_file("R5R Library/LIVE/r5apex.exe", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"MZ fake exe body").unwrap();

            // Kept sibling — contributes to compressed total.
            zw.start_file(
                "R5R Library/LIVE/paks/ui.rpak",
                SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(&[0u8; 2048]).unwrap();
        });

        let plan = plan_extraction(&p).unwrap();
        assert_eq!(plan.channel, "LIVE");
        assert_eq!(plan.kept_indices.len(), 2, "README.md must be excluded");
        assert!(
            plan.kept_compressed_total > 0,
            "kept_compressed_total should be > 0"
        );
    }
}
