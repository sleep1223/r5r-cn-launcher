use crate::config::paths::LIBRARY_DIR_NAME;
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

// ===== Auto-adopt existing R5Reloaded install =====

#[derive(Debug, Clone, Serialize)]
pub struct AdoptResult {
    pub adopted: bool,
    pub channel_dir: Option<String>,
    pub library_root: Option<String>,
}

/// Detect an existing R5Reloaded install (via registry / shortcut / library
/// scan) and, if found, write `library_root` + channel state into our settings
/// so the user can immediately hit "校验" without manual path selection.
///
/// The official R5Reloaded launcher uses the same `R5R Library/<CHANNEL>/`
/// directory structure as us, so we derive `library_root` by walking up from
/// the detected path past the `R5R Library` segment.
#[tauri::command]
pub async fn auto_adopt_existing_install(
    state: State<'_, LauncherState>,
) -> AppResult<AdoptResult> {
    // Skip if we already have any channel marked installed.
    {
        let s = state.settings.read();
        if s.channels.values().any(|c| c.installed) {
            return Ok(AdoptResult {
                adopted: false,
                channel_dir: None,
                library_root: None,
            });
        }
    }

    let extra = {
        let s = state.settings.read();
        let mut v = Vec::new();
        if !s.library_root.is_empty() {
            v.push(s.library_root.clone());
        }
        v
    };
    let found = detect_existing(&extra).await;

    // Pick the first detected install that actually has the game executable.
    let hit = match found.into_iter().find(|d| d.has_game) {
        Some(h) => h,
        None => {
            return Ok(AdoptResult {
                adopted: false,
                channel_dir: None,
                library_root: None,
            });
        }
    };

    // Derive library_root from the detected path. The R5Reloaded structure is
    // `<library_root>/R5R Library/<CHANNEL>/`, so we walk up looking for the
    // `R5R Library` segment.
    let channel_dir = std::path::PathBuf::from(&hit.path);
    let (library_root, channel_name) = match derive_library_root(&channel_dir) {
        Some(pair) => pair,
        None => {
            tracing::debug!(
                target: "detect",
                "cannot derive library_root from {}",
                hit.path
            );
            return Ok(AdoptResult {
                adopted: false,
                channel_dir: None,
                library_root: None,
            });
        }
    };

    // Write into our settings.
    {
        let mut s = state.settings.write();
        s.library_root = crate::util::display_slash(&library_root);
        if s.selected_channel.is_empty() {
            s.selected_channel = channel_name.clone();
        }
        let ch = s.channels.entry(channel_name.clone()).or_default();
        ch.installed = true;
    }
    let _ = state.save_settings();

    tracing::info!(
        target: "detect",
        "auto-adopted R5Reloaded install: channel={}, dir={}",
        channel_name,
        crate::util::display_slash(&channel_dir)
    );

    Ok(AdoptResult {
        adopted: true,
        channel_dir: Some(crate::util::display_slash(&channel_dir)),
        library_root: Some(crate::util::display_slash(&library_root)),
    })
}

/// Given a path like `D:\Games\R5R Library\LIVE`, return
/// `(D:\Games, "LIVE")`. Returns `None` if the path doesn't contain
/// the `R5R Library` segment.
fn derive_library_root(channel_dir: &std::path::Path) -> Option<(std::path::PathBuf, String)> {
    let components: Vec<_> = channel_dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    // Find the "R5R Library" segment (case-insensitive).
    for i in (0..components.len()).rev() {
        if components[i].eq_ignore_ascii_case(LIBRARY_DIR_NAME) {
            // The channel name is the segment after "R5R Library".
            let channel = components.get(i + 1)?.to_string();
            // library_root is everything before "R5R Library".
            let root: std::path::PathBuf = components[..i].iter().collect();
            if root.as_os_str().is_empty() {
                return None;
            }
            return Some((root, channel));
        }
    }
    None
}
