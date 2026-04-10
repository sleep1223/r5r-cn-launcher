use crate::detect::{detect_existing, DetectedInstall};
use crate::error::AppResult;
use crate::state::LauncherState;
use serde::Serialize;
use tauri::State;

#[tauri::command]
pub async fn detect_existing_r5r(
    state: State<'_, LauncherState>,
) -> AppResult<Vec<DetectedInstall>> {
    let extra = {
        let s = state.settings.read();
        let mut v = Vec::new();
        if !s.library_root.is_empty() {
            v.push(s.library_root.clone());
        }
        if let Some(p) = &s.last_known_official_install_path {
            v.push(p.clone());
        }
        v
    };
    let found = detect_existing(&extra).await;

    // If we discovered anything, remember the first one as the "official"
    // hint so the install picker can show it next time even when detection is
    // slow or offline.
    if let Some(first) = found.first() {
        let mut s = state.settings.write();
        s.last_known_official_install_path = Some(first.path.clone());
        drop(s);
        let _ = state.save_settings();
    }
    Ok(found)
}

// ===== Auto-adopt existing install =====

#[derive(Debug, Clone, Serialize)]
pub struct AdoptResult {
    /// Whether an existing LIVE install was found and adopted.
    pub adopted: bool,
    /// The path that was adopted (if any).
    pub channel_dir: Option<String>,
    /// The library root that was written to settings (parent of LIVE/).
    pub library_root: Option<String>,
    /// Version string from the official launcher settings, if any.
    pub game_version: Option<String>,
    /// True if the LIVE channel was already marked installed in our settings.
    pub was_already_adopted: bool,
}

/// Read the official R5Valkyrie launcher's `settings.json` to find an existing
/// LIVE install. If found AND we don't already have LIVE marked as installed,
/// write `library_root` + channel state into our own settings so the user can
/// immediately hit "校验" without manual path selection.
///
/// Returns an `AdoptResult` the frontend can use to decide whether to auto-
/// trigger verification.
#[tauri::command]
pub fn auto_adopt_existing_install(
    state: State<'_, LauncherState>,
) -> AppResult<AdoptResult> {
    // Already have LIVE installed? Skip — don't overwrite the user's state.
    {
        let s = state.settings.read();
        if s.channels.get("LIVE").map(|c| c.installed).unwrap_or(false) {
            return Ok(AdoptResult {
                adopted: false,
                channel_dir: None,
                library_root: None,
                game_version: None,
                was_already_adopted: true,
            });
        }
    }

    #[cfg(windows)]
    {
        use crate::detect::official_settings::read_official_live_install;
        let Some(official) = read_official_live_install() else {
            return Ok(AdoptResult {
                adopted: false,
                channel_dir: None,
                library_root: None,
                game_version: None,
                was_already_adopted: false,
            });
        };

        // Write into our settings: library_root + LIVE channel state.
        {
            let mut s = state.settings.write();
            s.library_root = official.library_root.display().to_string();
            if s.selected_channel.is_empty() {
                s.selected_channel = "LIVE".to_string();
            }
            let ch = s.channels.entry("LIVE".to_string()).or_default();
            ch.installed = true;
            if let Some(ver) = &official.game_version {
                ch.version = ver.clone();
            }
        }
        let _ = state.save_settings();

        tracing::info!(
            target: "detect",
            "auto-adopted official LIVE install at {}",
            official.channel_dir.display()
        );

        Ok(AdoptResult {
            adopted: true,
            channel_dir: Some(official.channel_dir.display().to_string()),
            library_root: Some(official.library_root.display().to_string()),
            game_version: official.game_version,
            was_already_adopted: false,
        })
    }

    #[cfg(not(windows))]
    {
        Ok(AdoptResult {
            adopted: false,
            channel_dir: None,
            library_root: None,
            game_version: None,
            was_already_adopted: false,
        })
    }
}
