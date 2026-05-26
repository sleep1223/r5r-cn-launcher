use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::offline::shape_detect::{friendly_zip_open_err, preflight_zip, zip_entry_keep};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// Counts bytes as they pass through a `Read`. Wrapping the source stream
/// (outside of any `BufReader`) gives us a true "bytes consumed" meter, which
/// the streaming importer uses as the progress denominator: compressed-byte
/// progress with ETA, without a CD prescan.
struct CountingReader<R: Read> {
    inner: R,
    counter: Arc<AtomicU64>,
}

impl<R: Read> CountingReader<R> {
    fn new(inner: R, counter: Arc<AtomicU64>) -> Self {
        Self { inner, counter }
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.counter.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

/// Streaming offline-zip import.
///
/// Reads the zip front-to-back via `read_zipfile_from_stream` — **does not**
/// require a readable EOCD / central directory. This is a deliberate design
/// choice: 20+ GB offline packs produced by some Chinese archivers have
/// non-standard EOCD layouts that zip-rs 2.x can't locate (often because the
/// comment region is larger than the 64 KB scan window, or ZIP64 locator is
/// written at a slight offset), yet their local file headers are fine and
/// the data is fully recoverable. v0.28's strict path rejected these packs
/// up front with a misleading "下载被截断" message.
///
/// Trade-offs vs the v0.28 CD-based path:
///   * No per-file count denominator — streaming doesn't know how many kept
///     entries exist until it's seen them all. `file_count` stays 0; the UI
///     already falls back to byte-progress when file_count is 0.
///   * Progress denominator is the raw zip file size (bytes consumed from
///     the source). Non-kept entries count toward the denominator too — we
///     still have to read past their compressed data to reach the next LFH.
///   * Mid-stream truncation surfaces at the moment we hit EOF instead of up
///     front. `preflight_zip` still catches wrong-magic / split-volume cases
///     before any bytes hit disk.
///   * No "准备中" stall — extraction starts immediately on open.
///
/// The anchor check (`R5R Library/<channel>/r5apex.exe`) runs at stream end:
/// if we never saw it, return a clear error so the user knows the pack is
/// incomplete or mis-structured. Files already written stay on disk —
/// the caller's subsequent Repair pass can reconcile against the manifest.
pub async fn import_zip_strict(
    app: &AppHandle,
    job_id: &str,
    zip_path: &Path,
    install_root: &Path,
    cancel: CancellationToken,
) -> AppResult<String> {
    preflight_zip(zip_path)?;

    let app_clone = app.clone();
    let jid = job_id.to_string();
    let zp = zip_path.to_path_buf();
    let ir = install_root.to_path_buf();
    let cancel_clone = cancel.clone();

    let channel = tokio::task::spawn_blocking(move || {
        extract_streaming(&zp, &ir, &jid, cancel_clone, |ev| {
            let _ = app_clone.emit(EVT_INSTALL_PROGRESS, ev);
        })
    })
    .await
    .map_err(|e| AppError::other(format!("zip 解压任务中断: {}", e)))??;

    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.to_string(), InstallPhase::Complete),
    );
    Ok(channel)
}

/// Core streaming extraction. Pure sync — the `spawn_blocking` wrapper lives
/// in the async caller. Takes an `emit` callback rather than an `AppHandle`
/// so tests can drive it without a Tauri runtime.
fn extract_streaming<F>(
    zip_path: &Path,
    install_root: &Path,
    job_id: &str,
    cancel: CancellationToken,
    mut emit: F,
) -> AppResult<String>
where
    F: FnMut(ProgressEvent),
{
    std::fs::create_dir_all(install_root)?;

    let file = std::fs::File::open(zip_path)?;
    let total = file.metadata().map(|m| m.len()).unwrap_or(0);
    // Buffer the raw file reads, then count bytes as the zip reader pulls
    // them out of the buffer. Counter outside the BufReader = progress
    // reflects bytes the zip parser has actually consumed, not just
    // bytes prefetched from disk.
    let buffered = BufReader::with_capacity(1024 * 1024, file);
    let counter = Arc::new(AtomicU64::new(0));
    let mut reader = CountingReader::new(buffered, counter.clone());

    // Emit once up-front so the UI flips past any prior Preparing state.
    emit(ProgressEvent {
        job_id: job_id.into(),
        phase: InstallPhase::Downloading,
        file_index: 0,
        file_count: 0,
        bytes_done: 0,
        bytes_total: total,
        current_file: String::new(),
        speed_bps: 0,
        eta_seconds: 0,
    });

    let mut channel: Option<String> = None;
    let mut file_index: usize = 0;
    let started = Instant::now();
    let mut last_emit = Instant::now();

    loop {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }

        let mut entry = match zip::read::read_zipfile_from_stream(&mut reader) {
            Ok(Some(e)) => e,
            Ok(None) => break, // Reached central directory — normal end of stream.
            Err(zip::result::ZipError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // Stream truncated mid-entry. Let the post-condition (anchor
                // seen?) decide whether this is a hard failure; a complete
                // game body with a truncated tail is still installable via
                // Repair, whereas a pack that cut off before r5apex.exe is
                // unusable.
                break;
            }
            Err(e) => {
                return Err(AppError::other(format!("zip 流读取失败: {}", e)));
            }
        };

        if entry.is_dir() {
            continue; // Drop exhausts the (zero-length) entry data.
        }
        let name = entry.name().to_string();
        if !zip_entry_keep(&name) {
            continue; // Drop skips over the entry's compressed bytes.
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

        // `enclosed_name` rejects absolute paths and `..` traversal; None
        // means we should skip (don't create anything outside install_root).
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

            if last_emit.elapsed().as_millis() > 200 {
                last_emit = Instant::now();
                let done = counter.load(Ordering::Relaxed).min(total);
                let elapsed = started.elapsed().as_secs_f64().max(0.001);
                let speed = (done as f64 / elapsed) as u64;
                let eta = if speed > 0 {
                    total.saturating_sub(done) / speed
                } else {
                    0
                };
                emit(ProgressEvent {
                    job_id: job_id.into(),
                    phase: InstallPhase::Downloading,
                    file_index,
                    file_count: 0,
                    bytes_done: done,
                    bytes_total: total,
                    current_file: crate::util::display_slash(&rel_buf),
                    speed_bps: speed,
                    eta_seconds: eta,
                });
            }
        }
        out.sync_all()?;
        file_index += 1;
    }

    channel.ok_or_else(|| {
        AppError::InvalidPath(
            "zip 流结束但未发现 R5R Library/<频道>/r5apex.exe。\
             可能是分卷包只含半壁（未合并前导卷），或压缩包在 r5apex.exe 之前就已截断。\
             请确认来源完整性后重试，或改用【选择已解压的目录】导入。"
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
///
/// **Used by the patch path only** (`download/patch.rs`). Patches are small
/// (< 500 MB typically) and we can afford the CD walk there; the offline
/// import path uses the streaming `import_zip_strict` above because 20+ GB
/// packs often have EOCD issues the CD walk can't handle.
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
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let mut archive = zip::ZipArchive::new(f).map_err(|e| friendly_zip_open_err(size, &e))?;

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

    /// Pure-sync driver for the streaming extractor, for test use.
    fn run_stream(zip_path: &Path, install_root: &Path) -> AppResult<String> {
        extract_streaming(
            zip_path,
            install_root,
            "test",
            CancellationToken::new(),
            |_| {},
        )
    }

    #[test]
    fn streaming_extracts_and_returns_channel() {
        let td = tempdir().unwrap();
        let zp = td.path().join("ok.zip");
        write_zip(&zp, |zw| {
            // Non-kept entry at the top — streaming must skip past it.
            zw.start_file("README.md", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"top-level readme").unwrap();

            zw.start_file("R5R Library/LIVE/r5apex.exe", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"MZ fake exe body").unwrap();

            zw.start_file(
                "R5R Library/LIVE/paks/ui.rpak",
                SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(&[0xABu8; 2048]).unwrap();
        });

        let out = td.path().join("out");
        let channel = run_stream(&zp, &out).unwrap();
        assert_eq!(channel, "LIVE");
        assert!(out.join("R5R Library/LIVE/r5apex.exe").exists());
        assert!(out.join("R5R Library/LIVE/paks/ui.rpak").exists());
        // Non-kept entry must not land in the install root.
        assert!(!out.join("README.md").exists());
    }

    #[test]
    fn streaming_rejects_pack_without_anchor() {
        let td = tempdir().unwrap();
        let zp = td.path().join("no_anchor.zip");
        write_zip(&zp, |zw| {
            zw.start_file("R5R Library/LIVE/readme.txt", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"hello").unwrap();
        });

        let out = td.path().join("out");
        let err = run_stream(&zp, &out).unwrap_err();
        assert!(matches!(err, AppError::InvalidPath(_)), "got {:?}", err);
        assert!(err.to_string().contains("r5apex.exe"), "got {}", err);
    }

    /// The core win of the streaming approach: a pack whose EOCD zip-rs
    /// can't locate (here simulated by a truncated tail) still extracts up
    /// to where the stream died. If the anchor was seen, the import
    /// succeeds and the caller can run Repair to reconcile the rest.
    #[test]
    fn streaming_survives_missing_eocd_after_anchor() {
        let td = tempdir().unwrap();
        let full = td.path().join("full.zip");
        write_zip(&full, |zw| {
            zw.start_file("R5R Library/LIVE/r5apex.exe", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"MZ fake exe body").unwrap();

            zw.start_file(
                "R5R Library/LIVE/paks/ui.rpak",
                SimpleFileOptions::default(),
            )
            .unwrap();
            zw.write_all(&[0x55u8; 256]).unwrap();
        });

        // Chop off the tail — CD + EOCD gone. Pick a cut point that lands
        // well after the second file's compressed data so both entries are
        // fully recoverable from the LFH stream.
        let full_bytes = std::fs::read(&full).unwrap();
        let chopped = td.path().join("chopped.zip");
        let cut = full_bytes.len().saturating_sub(120);
        std::fs::write(&chopped, &full_bytes[..cut]).unwrap();

        let out = td.path().join("out");
        let channel = run_stream(&chopped, &out).unwrap();
        assert_eq!(channel, "LIVE");
        assert!(out.join("R5R Library/LIVE/r5apex.exe").exists());
    }
}
