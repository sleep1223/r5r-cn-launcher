//! Auto-detect the layout of an offline pack so we know what to copy where.
//!
//! Accepted shapes:
//!   1. Directory that IS `<.../R5R Library>` — picked dir contains channel folders.
//!   2. Directory that CONTAINS `R5R Library/...` — one level above (1).
//!   3. Directory that IS a single channel folder (`<...>/r5apex.exe` exists at root).
//!   4. Zip file containing any of the above.
//!
//! Anything else is rejected with a clear error so the user can fix the pack.

use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// What we extracted from inspecting the pack.
#[derive(Debug, Clone)]
pub struct DetectedShape {
    /// The source root we should copy/extract from.
    pub source_root: PathBuf,
    /// The channel name (folder name we'll create under `R5R Library/`).
    pub channel: String,
}

/// Inspect a directory and figure out where the R5R content actually starts.
pub fn detect_directory(picked: &Path) -> AppResult<DetectedShape> {
    if !picked.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "{} 不是一个目录",
            picked.display()
        )));
    }

    // Case 3: picked dir IS a channel — has r5apex.exe at root.
    if picked.join("r5apex.exe").exists() {
        let channel = picked
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "LIVE".into());
        return Ok(DetectedShape {
            source_root: picked.to_path_buf(),
            channel,
        });
    }

    // Case 1: picked dir IS `R5R Library` (case-insensitive).
    let name = picked
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if name.eq_ignore_ascii_case("R5R Library") {
        let channel = single_channel_in(picked)?;
        return Ok(DetectedShape {
            source_root: picked.join(&channel),
            channel,
        });
    }

    // Case 2: picked dir CONTAINS `R5R Library/`.
    let lib = find_child_named(picked, "R5R Library");
    if let Some(lib) = lib {
        let channel = single_channel_in(&lib)?;
        return Ok(DetectedShape {
            source_root: lib.join(&channel),
            channel,
        });
    }

    Err(AppError::InvalidPath(
        "未能识别离线包结构。请确认目录中包含 `R5R Library/<频道>/r5apex.exe`。".into(),
    ))
}

/// Inspect a zip file and figure out the strip prefix, channel, total
/// uncompressed bytes, and file count — all by reading the in-memory central
/// directory. Does NOT call `by_index`, which seeks to every entry's local
/// header and was the reason "准备中" hung for many seconds on 30 GB packs.
pub fn detect_zip(
    app: &AppHandle,
    job_id: &str,
    cancel: &CancellationToken,
    zip_path: &Path,
) -> AppResult<DetectedZipShape> {
    emit_scan_progress(app, job_id);

    // ZipArchive::new reads the EOCD + full central directory once. For a
    // 30 GB pack with ~50k entries the CD itself is only a few MB, so this is
    // a single linear read — seconds at worst on HDD, milliseconds on SSD.
    let f = std::fs::File::open(zip_path)?;
    let archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::other(format!("无法打开 zip: {}", e)))?;

    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }

    // Pure in-memory scans from here down.
    let anchor = archive
        .file_names()
        .find(|n| {
            let l = n.to_ascii_lowercase();
            l.ends_with("/r5apex.exe") || l == "r5apex.exe"
        })
        .ok_or_else(|| {
            AppError::InvalidPath(
                "zip 包中未找到 r5apex.exe，请确认这是一个有效的 R5R 离线包。".into(),
            )
        })?
        .to_string();

    let parts: Vec<&str> = anchor.split('/').collect();
    if parts.len() < 2 {
        return Err(AppError::InvalidPath(format!(
            "zip 内的 r5apex.exe 路径无效: {}",
            anchor
        )));
    }
    let channel = parts[parts.len() - 2].to_string();
    let strip_prefix = parts[..parts.len() - 1].join("/") + "/";

    let file_count = archive
        .file_names()
        .filter(|n| n.starts_with(&strip_prefix) && !n.ends_with('/'))
        .count();

    // decompressed_size() sums uncompressed sizes straight from the central
    // directory — no I/O. Returns None for data-descriptor zips (rare); in
    // that case fall back to 0 and the UI switches to file-count-based
    // progress.
    let total_bytes = archive.decompressed_size().map(|v| v as u64).unwrap_or(0);

    Ok(DetectedZipShape {
        strip_prefix,
        channel,
        total_bytes,
        file_count,
    })
}

fn emit_scan_progress(app: &AppHandle, job_id: &str) {
    let _ = app.emit(
        EVT_INSTALL_PROGRESS,
        ProgressEvent::empty(job_id.to_string(), InstallPhase::Preparing),
    );
}

#[derive(Debug, Clone)]
pub struct DetectedZipShape {
    /// Path prefix inside the zip that should be stripped before extracting,
    /// e.g. `R5R Library/LIVE/`. Entries that don't start with this prefix
    /// are ignored.
    pub strip_prefix: String,
    pub channel: String,
    /// Total uncompressed bytes across entries matching the prefix — filled
    /// in by `detect_zip` so the importer can skip a second full scan.
    pub total_bytes: u64,
    /// Number of matching regular-file entries.
    pub file_count: usize,
}

fn single_channel_in(dir: &Path) -> AppResult<String> {
    let mut candidates: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().join("r5apex.exe").exists() {
            candidates.push(name);
        }
    }
    match candidates.len() {
        0 => Err(AppError::InvalidPath(
            "目录中未找到包含 r5apex.exe 的频道文件夹。".into(),
        )),
        1 => Ok(candidates.into_iter().next().unwrap()),
        _ => Err(AppError::InvalidPath(format!(
            "目录中存在多个频道：{}。请只保留一个或单独导入。",
            candidates.join(", ")
        ))),
    }
}

fn find_child_named(dir: &Path, name: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    for entry in rd.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        if p.file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
            .unwrap_or(false)
        {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn picked_dir_is_channel() {
        let td = tempdir().unwrap();
        let chan = td.path().join("LIVE");
        std::fs::create_dir(&chan).unwrap();
        std::fs::write(chan.join("r5apex.exe"), b"x").unwrap();
        let shape = detect_directory(&chan).unwrap();
        assert_eq!(shape.channel, "LIVE");
        assert_eq!(shape.source_root, chan);
    }

    #[test]
    fn picked_dir_is_r5r_library() {
        let td = tempdir().unwrap();
        let lib = td.path().join("R5R Library");
        let chan = lib.join("LIVE");
        std::fs::create_dir_all(&chan).unwrap();
        std::fs::write(chan.join("r5apex.exe"), b"x").unwrap();
        let shape = detect_directory(&lib).unwrap();
        assert_eq!(shape.channel, "LIVE");
        assert_eq!(shape.source_root, chan);
    }

    #[test]
    fn picked_dir_contains_r5r_library() {
        let td = tempdir().unwrap();
        let chan = td.path().join("R5R Library").join("LIVE");
        std::fs::create_dir_all(&chan).unwrap();
        std::fs::write(chan.join("r5apex.exe"), b"x").unwrap();
        let shape = detect_directory(td.path()).unwrap();
        assert_eq!(shape.channel, "LIVE");
        assert_eq!(shape.source_root, chan);
    }

    #[test]
    fn ambiguous_multiple_channels_rejected() {
        let td = tempdir().unwrap();
        let lib = td.path().join("R5R Library");
        std::fs::create_dir_all(lib.join("LIVE")).unwrap();
        std::fs::create_dir_all(lib.join("STABLE")).unwrap();
        std::fs::write(lib.join("LIVE").join("r5apex.exe"), b"x").unwrap();
        std::fs::write(lib.join("STABLE").join("r5apex.exe"), b"x").unwrap();
        let r = detect_directory(&lib);
        assert!(r.is_err());
    }
}
