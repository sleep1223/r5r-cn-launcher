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

    let path = match install_dir {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    Ok(vec![DetectedInstall {
        source: DetectSource::Shortcut,
        path,
        channel: None,
        version: None,
    }])
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
