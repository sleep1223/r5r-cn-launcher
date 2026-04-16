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
use std::io::Read;
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
            crate::util::display_slash(picked)
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

/// Cheap pre-checks that run before we hand the file to zip-rs. The EOCD
/// error zip-rs produces is opaque — a real 20 GB pack usually fails it
/// for one of three reasons, all of which we can detect or hint at without
/// actually parsing the zip: truncated download, a non-zip format renamed
/// to .zip, or a split-volume archive. Each branch below maps to a single
/// one of those with a concrete recovery action for the user.
pub fn preflight_zip(zip_path: &Path) -> AppResult<()> {
    let meta = std::fs::metadata(zip_path)?;
    let size = meta.len();

    // Minimum zip is a 22-byte EOCD record. Anything smaller is garbage.
    if size < 22 {
        return Err(AppError::InvalidPath(format!(
            "文件过小（{} 字节），无法构成有效的 zip。请确认下载是否完整。",
            size
        )));
    }

    // Peek the first 4 bytes. A valid zip starts with PK\x03\x04 (local file
    // header), PK\x05\x06 (empty archive EOCD), or PK\x07\x08 (spanned
    // marker). Other magics mean the .zip extension is lying.
    let mut f = std::fs::File::open(zip_path)?;
    let mut magic = [0u8; 4];
    let n = f.read(&mut magic)?;
    if n >= 4 {
        match magic {
            [0x50, 0x4b, 0x03, 0x04]
            | [0x50, 0x4b, 0x05, 0x06]
            | [0x50, 0x4b, 0x07, 0x08] => {}
            // 7z archive signature (37 7A BC AF 27 1C)
            [0x37, 0x7a, 0xbc, 0xaf] => {
                return Err(AppError::InvalidPath(
                    "这实际上是 7z 压缩包（扩展名被改成了 .zip）。\
                     请先用 7-Zip / Bandizip 解压为目录，再用【选择已解压的目录】导入。"
                        .into(),
                ));
            }
            // RAR archive signatures ("Rar!" for RAR4, same 4 bytes for RAR5)
            [0x52, 0x61, 0x72, 0x21] => {
                return Err(AppError::InvalidPath(
                    "这实际上是 RAR 压缩包（扩展名被改成了 .zip）。\
                     请先用 WinRAR / Bandizip 解压为目录，再用【选择已解压的目录】导入。"
                        .into(),
                ));
            }
            // MZ = Windows PE. Self-extracting exes sometimes wear .zip.
            [0x4d, 0x5a, _, _] => {
                return Err(AppError::InvalidPath(
                    "这实际上是一个 Windows 可执行文件（例如自解压安装器），\
                     不是 zip。请直接运行它解压，或向下载来源确认文件。"
                        .into(),
                ));
            }
            _ => {
                return Err(AppError::InvalidPath(format!(
                    "文件头不是 zip 签名（实际: {:02x} {:02x} {:02x} {:02x}）。\
                     可能是下载被截断，或文件被改名。请重新下载或用 7-Zip 试解。",
                    magic[0], magic[1], magic[2], magic[3]
                )));
            }
        }
    }

    // Split-volume companions — zip-rs 2.x does not read multi-volume sets.
    // Check for both naming conventions used by Windows archivers.
    if let Some(parent) = zip_path.parent() {
        let stem = zip_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let filename = zip_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // WinZip/Bandizip split: foo.zip + foo.z01 + foo.z02 ...
        let z01 = parent.join(format!("{}.z01", stem));
        if z01.exists() {
            return Err(AppError::InvalidPath(format!(
                "检测到分卷压缩包（存在 {}）。\
                 本工具不支持分卷 zip —— 请先用 7-Zip / Bandizip 合并为单个 .zip，\
                 或解压成目录后用【选择已解压的目录】导入。",
                z01.file_name().unwrap().to_string_lossy()
            )));
        }

        // 7-Zip split: foo.zip.001 + foo.zip.002 ...
        let split_001 = parent.join(format!("{}.001", filename));
        if split_001.exists() {
            return Err(AppError::InvalidPath(format!(
                "检测到分卷压缩包（存在 {}）。\
                 本工具不支持分卷 zip —— 请先用 7-Zip / Bandizip 合并为单个 .zip，\
                 或解压成目录后用【选择已解压的目录】导入。",
                split_001.file_name().unwrap().to_string_lossy()
            )));
        }
    }

    Ok(())
}

/// Turn zip-rs's opaque "Could not find EOCD" / "invalid Zip archive" text
/// into actionable Chinese guidance. Preflight already catches the common
/// causes; this runs only when preflight passed but the central directory
/// still fails to parse — usually truncation that happened in the middle of
/// the file, or a zip with trailing garbage that pushes EOCD out of the
/// default search window.
pub(crate) fn friendly_zip_open_err(size: u64, e: &zip::result::ZipError) -> AppError {
    let raw = e.to_string();
    let gb = size as f64 / 1024.0 / 1024.0 / 1024.0;
    if raw.contains("EOCD") || raw.contains("central directory") {
        AppError::InvalidPath(format!(
            "无法在 zip 末尾定位结构记录（EOCD）。文件大小：{:.2} GB。\n\
             可能原因：\n  \
               1. 下载未完成或被截断 —— 请对照镜像站的 SHA/MD5 校验后重新下载；\n  \
               2. 是分卷压缩包（.z01 / .zip.001）—— 请先合并为单个 .zip；\n  \
               3. 工具在尾部附加了非标准数据 —— 建议先解压为目录后用\
                 【选择已解压的目录】导入。",
            gb
        ))
    } else {
        AppError::other(format!("无法打开 zip: {}", raw))
    }
}

/// Lightweight zip scan for the patch path. Patches may not ship a full
/// `r5apex.exe` so the full detection check would reject them — here we
/// just count kept-prefix files without the anchor requirement.
pub fn scan_zip_kept_entries(zip_path: &Path) -> AppResult<(u64, usize)> {
    preflight_zip(zip_path)?;
    let f = std::fs::File::open(zip_path)?;
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    let archive = zip::ZipArchive::new(f).map_err(|e| friendly_zip_open_err(size, &e))?;
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

    #[test]
    fn preflight_rejects_tiny_file() {
        let td = tempdir().unwrap();
        let p = td.path().join("tiny.zip");
        std::fs::write(&p, b"PK").unwrap();
        let err = preflight_zip(&p).unwrap_err();
        assert!(matches!(err, AppError::InvalidPath(_)), "got {:?}", err);
        assert!(err.to_string().contains("过小"));
    }

    #[test]
    fn preflight_rejects_7z_magic() {
        let td = tempdir().unwrap();
        let p = td.path().join("fake.zip");
        // 7z signature + enough padding to clear the 22-byte min
        let mut buf = vec![0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c];
        buf.extend(std::iter::repeat(0u8).take(64));
        std::fs::write(&p, buf).unwrap();
        let err = preflight_zip(&p).unwrap_err();
        assert!(err.to_string().contains("7z"));
    }

    #[test]
    fn preflight_rejects_rar_magic() {
        let td = tempdir().unwrap();
        let p = td.path().join("fake.zip");
        let mut buf = vec![0x52, 0x61, 0x72, 0x21, 0x1a, 0x07];
        buf.extend(std::iter::repeat(0u8).take(64));
        std::fs::write(&p, buf).unwrap();
        let err = preflight_zip(&p).unwrap_err();
        assert!(err.to_string().contains("RAR"));
    }

    #[test]
    fn preflight_flags_z01_split_companion() {
        let td = tempdir().unwrap();
        let p = td.path().join("pack.zip");
        // Valid zip magic so we get past the signature gate.
        let mut buf = vec![0x50, 0x4b, 0x03, 0x04];
        buf.extend(std::iter::repeat(0u8).take(64));
        std::fs::write(&p, buf).unwrap();
        // Companion volume in the same directory — should trigger rejection.
        std::fs::write(td.path().join("pack.z01"), b"x").unwrap();
        let err = preflight_zip(&p).unwrap_err();
        assert!(err.to_string().contains("分卷"), "got {}", err);
    }

    #[test]
    fn preflight_flags_dot_001_split_companion() {
        let td = tempdir().unwrap();
        let p = td.path().join("pack.zip");
        let mut buf = vec![0x50, 0x4b, 0x03, 0x04];
        buf.extend(std::iter::repeat(0u8).take(64));
        std::fs::write(&p, buf).unwrap();
        std::fs::write(td.path().join("pack.zip.001"), b"x").unwrap();
        let err = preflight_zip(&p).unwrap_err();
        assert!(err.to_string().contains("分卷"), "got {}", err);
    }

    #[test]
    fn preflight_passes_for_real_zip_signature() {
        let td = tempdir().unwrap();
        let p = td.path().join("ok.zip");
        let mut buf = vec![0x50, 0x4b, 0x03, 0x04];
        buf.extend(std::iter::repeat(0u8).take(64));
        std::fs::write(&p, buf).unwrap();
        // No companions and valid magic — preflight should accept (actual
        // zip-rs parse will fail later, but that's not preflight's job).
        assert!(preflight_zip(&p).is_ok());
    }
}
