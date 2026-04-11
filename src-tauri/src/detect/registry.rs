use crate::detect::{DetectSource, DetectedInstall};
use anyhow::Result;
use winreg::enums::*;
use winreg::RegKey;

const UNINSTALL_PATHS: &[&str] = &[
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
];

pub fn detect() -> Result<Vec<DetectedInstall>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut found = Vec::new();

    for &base in UNINSTALL_PATHS {
        let Ok(uninstall) = hklm.open_subkey(base) else {
            continue;
        };
        for sub_name in uninstall.enum_keys().flatten() {
            let Ok(sub) = uninstall.open_subkey(&sub_name) else {
                continue;
            };
            let display_name: String = sub.get_value("DisplayName").unwrap_or_default();
            if !display_name.to_ascii_lowercase().contains("r5reloaded") {
                continue;
            }
            let install_location: String =
                sub.get_value("InstallLocation").unwrap_or_default();
            let display_icon: String = sub.get_value("DisplayIcon").unwrap_or_default();
            let display_version: String =
                sub.get_value("DisplayVersion").unwrap_or_default();

            let path = if !install_location.is_empty() {
                install_location
            } else if !display_icon.is_empty() {
                derive_dir_from_icon(&display_icon)
            } else {
                continue;
            };

            found.push(DetectedInstall {
                source: DetectSource::Registry,
                path,
                channel: None,
                version: if display_version.is_empty() {
                    None
                } else {
                    Some(display_version)
                },
                has_game: false,
            });
        }
    }
    Ok(found)
}

/// `DisplayIcon` is usually `C:\path\to\thing.exe,0`. Strip the `,N` suffix
/// and return the parent directory.
fn derive_dir_from_icon(icon: &str) -> String {
    let no_index = icon.rsplit_once(',').map(|(p, _)| p).unwrap_or(icon);
    std::path::Path::new(no_index)
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| no_index.to_string())
}
