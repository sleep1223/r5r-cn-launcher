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

/// Inspect a zip file and figure out the strip prefix and channel.
pub fn detect_zip(zip_path: &Path) -> AppResult<DetectedZipShape> {
    use std::fs::File;
    let f = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(f)
        .map_err(|e| AppError::other(format!("无法打开 zip: {}", e)))?;

    // Find the first entry whose path contains `r5apex.exe` — that anchors
    // the channel directory.
    let mut anchor_inside_zip: Option<String> = None;
    for i in 0..archive.len() {
        let e = archive
            .by_index(i)
            .map_err(|e| AppError::other(format!("读取 zip 条目失败: {}", e)))?;
        let name = e.name().to_string();
        if name.to_ascii_lowercase().ends_with("/r5apex.exe")
            || name.to_ascii_lowercase() == "r5apex.exe"
        {
            anchor_inside_zip = Some(name);
            break;
        }
    }

    let anchor = anchor_inside_zip.ok_or_else(|| {
        AppError::InvalidPath(
            "zip 包中未找到 r5apex.exe，请确认这是一个有效的 R5R 离线包。".into(),
        )
    })?;

    // anchor is something like "R5R Library/LIVE/r5apex.exe" or "LIVE/r5apex.exe".
    // The component immediately before "r5apex.exe" is the channel folder, and
    // everything before that is the strip prefix.
    let parts: Vec<&str> = anchor.split('/').collect();
    if parts.len() < 2 {
        return Err(AppError::InvalidPath(format!(
            "zip 内的 r5apex.exe 路径无效: {}",
            anchor
        )));
    }
    let channel = parts[parts.len() - 2].to_string();
    let strip_prefix = parts[..parts.len() - 1].join("/") + "/";

    Ok(DetectedZipShape {
        strip_prefix,
        channel,
    })
}

#[derive(Debug, Clone)]
pub struct DetectedZipShape {
    /// Path prefix inside the zip that should be stripped before extracting,
    /// e.g. `R5R Library/LIVE/`. Entries that don't start with this prefix
    /// are ignored.
    pub strip_prefix: String,
    pub channel: String,
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
