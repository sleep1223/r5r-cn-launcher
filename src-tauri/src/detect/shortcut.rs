use crate::detect::{DetectSource, DetectedInstall};
use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Locate `R5Reloaded.lnk` under `%ProgramData%\Microsoft\Windows\Start Menu\Programs\R5Reloaded\`,
/// parse it, and return the install dir it points at.
pub fn detect() -> Result<Vec<DetectedInstall>> {
    let lnk_path = shortcut_path()?;
    if !lnk_path.exists() {
        return Ok(Vec::new());
    }
    let link = lnk::ShellLink::open(&lnk_path)
        .map_err(|e| anyhow!("解析 R5Reloaded.lnk 失败: {:?}", e))?;

    // Prefer the working directory — that's the actual install dir for an
    // R5Reloaded shortcut. Fall back to the dirname of the link target if
    // working dir is missing.
    let install_dir = link
        .working_dir()
        .clone()
        .or_else(|| {
            link.relative_path()
                .clone()
                .and_then(|rp| PathBuf::from(rp).parent().map(|p| p.display().to_string()))
        });

    let raw_path = match install_dir {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    // The shortcut's working directory points at the launcher subfolder
    // (`...\R5Reloaded\R5R Launcher`), but users expect the actual game root.
    // Strip the trailing `R5R Launcher` segment if present.
    let path = strip_launcher_suffix(&raw_path);

    Ok(vec![DetectedInstall {
        source: DetectSource::Shortcut,
        path,
        channel: None,
        version: None,
    }])
}

/// Drop a trailing `R5R Launcher` directory segment, case-insensitively.
fn strip_launcher_suffix(p: &str) -> String {
    let trimmed = p.trim_end_matches(['\\', '/']);
    let mut segs: Vec<&str> = trimmed.split(['\\', '/']).collect();
    if segs
        .last()
        .map(|s| s.eq_ignore_ascii_case("R5R Launcher"))
        .unwrap_or(false)
    {
        segs.pop();
        return segs.join("\\");
    }
    p.to_string()
}

fn shortcut_path() -> Result<PathBuf> {
    use known_folders::{get_known_folder_path, KnownFolder};
    let program_data = get_known_folder_path(KnownFolder::ProgramData)
        .ok_or_else(|| anyhow!("无法获取 ProgramData 路径"))?;
    Ok(program_data
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("R5Reloaded")
        .join("R5Reloaded.lnk"))
}
