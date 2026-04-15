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
use std::path::{Path, PathBuf};
use tauri::AppHandle;
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

/// Top-level folders inside the zip that we extract. Everything else (README
/// blurbs, build-specific scripts, etc.) is silently skipped.
pub const ZIP_KEEP_PREFIXES: &[&str] = &["R5R Library/", "R5R Launcher/"];

/// Check whether a zip entry name falls under one of the keep-prefixes.
/// Case-insensitive match on the prefix; the remainder is preserved as-is.
pub fn zip_entry_keep(name: &str) -> bool {
    ZIP_KEEP_PREFIXES
        .iter()
        .any(|p| name.len() >= p.len() && name[..p.len()].eq_ignore_ascii_case(p))
}

/// Inspect a zip file and figure out the channel, total uncompressed bytes,
/// and file count for the subset of entries under `R5R Library/` and
/// `R5R Launcher/` — all by reading the in-memory central directory. Does
/// NOT call `by_index`, which seeks to every entry's local header and was
/// the reason "准备中" hung for many seconds on 30 GB packs.
pub fn detect_zip(
    _app: &AppHandle,
    _job_id: &str,
    cancel: &CancellationToken,
    zip_path: &Path,
) -> AppResult<DetectedZipShape> {
    // ZipArchive::new reads the EOCD + full central directory once. For a
    // 30 GB pack with ~50k entries the CD itself is only a few MB, so this is
    // a single linear read — seconds at worst on HDD, milliseconds on SSD.
    let f = std::fs::File::open(zip_path)?;
    let archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::other(format!("无法打开 zip: {}", e)))?;

    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }

    // Pure in-memory scans from here down. Find a `R5R Library/<channel>/r5apex.exe`
    // to nail down the channel name for settings updates.
    let anchor = archive
        .file_names()
        .find(|n| {
            let l = n.to_ascii_lowercase();
            l.starts_with("r5r library/") && l.ends_with("/r5apex.exe")
        })
        .ok_or_else(|| {
            AppError::InvalidPath(
                "zip 包中未找到 R5R Library/<频道>/r5apex.exe，请确认这是一个有效的 R5R 离线包。"
                    .into(),
            )
        })?
        .to_string();

    let parts: Vec<&str> = anchor.split('/').collect();
    if parts.len() < 3 {
        return Err(AppError::InvalidPath(format!(
            "zip 内的 r5apex.exe 路径无效: {}",
            anchor
        )));
    }
    let channel = parts[parts.len() - 2].to_string();

    // File count across kept prefixes — `file_names()` is purely in-memory.
    let file_count = archive
        .file_names()
        .filter(|n| !n.ends_with('/') && zip_entry_keep(n))
        .count();

    // Total bytes: archive-level sum from the central directory. When most of
    // the zip is game content (the typical case), the over-count from any
    // top-level README-style extras is negligible — and on data-descriptor
    // zips this returns None, in which case the UI falls back to file-count
    // progress.
    let total_bytes = archive.decompressed_size().map(|v| v as u64).unwrap_or(0);

    Ok(DetectedZipShape {
        channel,
        total_bytes,
        file_count,
    })
}

#[derive(Debug, Clone)]
pub struct DetectedZipShape {
    /// Channel name inferred from `R5R Library/<channel>/r5apex.exe` — used
    /// to update `selected_channel` in settings after import.
    pub channel: String,
    /// Total uncompressed bytes across kept entries (best-effort; may be 0
    /// for data-descriptor zips, in which case the UI falls back to
    /// file-count-based progress).
    pub total_bytes: u64,
    /// Number of regular-file entries under `R5R Library/` or `R5R Launcher/`.
    pub file_count: usize,
}

/// Lightweight zip scan for the patch path. Patches may not ship a full
/// `r5apex.exe` so the full `detect_zip` check would reject them — here we
/// just count kept-prefix files without the anchor requirement.
pub fn scan_zip_kept_entries(zip_path: &Path) -> AppResult<(u64, usize)> {
    let f = std::fs::File::open(zip_path)?;
    let archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::other(format!("无法打开 zip: {}", e)))?;
    let file_count = archive
        .file_names()
        .filter(|n| !n.ends_with('/') && zip_entry_keep(n))
        .count();
    let total_bytes = archive.decompressed_size().map(|v| v as u64).unwrap_or(0);
    Ok((total_bytes, file_count))
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
