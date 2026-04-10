//! Read the official R5Valkyrie launcher's `settings.json` to discover an
//! existing LIVE install — exactly what their launcher persists after a
//! successful download.
//!
//! Settings location: `%APPDATA%\r5vlauncher\settings.json`
//!
//! Relevant JSON shape:
//! ```json
//! {
//!   "installDir": "C:\\...\\R5VLibrary",        // custom base, optional
//!   "channels": {
//!     "LIVE": {
//!       "installDir": "C:\\...\\R5VLibrary\\LIVE",
//!       "gameVersion": "2.6.41",
//!       "lastUpdatedAt": 1712345678901
//!     }
//!   }
//! }
//! ```
//!
//! We don't import their full schema; we only need the two fields above.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Minimal subset of the official launcher's settings that we care about.
#[derive(Debug, Deserialize, Default)]
struct OfficialSettings {
    #[serde(default, rename = "installDir")]
    install_dir: Option<String>,
    #[serde(default)]
    channels: HashMap<String, OfficialChannelState>,
}

#[derive(Debug, Deserialize, Default)]
struct OfficialChannelState {
    #[serde(default, rename = "installDir")]
    install_dir: Option<String>,
    #[serde(default, rename = "gameVersion")]
    game_version: Option<String>,
}

/// Result of reading the official launcher's settings.
#[derive(Debug, Clone)]
pub struct OfficialInstall {
    /// Full path to the channel dir (e.g. `D:\R5VLibrary\LIVE`).
    pub channel_dir: PathBuf,
    /// Channel name, uppercase (e.g. `"LIVE"`).
    pub channel_name: String,
    /// Game version from the official settings, if present.
    pub game_version: Option<String>,
    /// The library root (parent of `LIVE/` dir).
    pub library_root: PathBuf,
}

/// Try to read the official R5V launcher's settings.json and locate the
/// LIVE channel install. Returns `None` if the file doesn't exist, can't be
/// parsed, or the LIVE channel has no install dir recorded.
pub fn read_official_live_install() -> Option<OfficialInstall> {
    let settings_path = official_settings_path()?;
    if !settings_path.exists() {
        tracing::debug!(target: "detect", "official settings not found at {}", settings_path.display());
        return None;
    }
    let bytes = std::fs::read(&settings_path).ok()?;
    let cfg: OfficialSettings = serde_json::from_slice(&bytes).ok()?;

    // 1. Try the channel-specific installDir for LIVE.
    let live = cfg.channels.get("LIVE");
    let channel_dir = live
        .and_then(|ch| ch.install_dir.as_deref())
        .map(PathBuf::from)
        .or_else(|| {
            // 2. Fallback: base installDir + "LIVE"
            cfg.install_dir.as_deref().map(|base| PathBuf::from(base).join("LIVE"))
        })
        .or_else(|| {
            // 3. Fallback: platform default + "LIVE"
            default_library_base().map(|base| base.join("LIVE"))
        })?;

    if !channel_dir.exists() {
        tracing::debug!(target: "detect", "official LIVE dir {} doesn't exist", channel_dir.display());
        return None;
    }

    // Verify that r5apex.exe exists (same check the official launcher does).
    let has_game = channel_dir.join("r5apex.exe").exists()
        || channel_dir.join("r5apex_ds.exe").exists();
    if !has_game {
        tracing::debug!(target: "detect", "no r5apex.exe in {}", channel_dir.display());
        return None;
    }

    // Derive library_root = parent of the channel dir (e.g. D:\R5VLibrary).
    let library_root = channel_dir.parent()?.to_path_buf();

    let game_version = live.and_then(|ch| ch.game_version.clone());

    tracing::info!(
        target: "detect",
        "found official LIVE install at {} (version {:?})",
        channel_dir.display(),
        game_version,
    );

    Some(OfficialInstall {
        channel_dir,
        channel_name: "LIVE".to_string(),
        game_version,
        library_root,
    })
}

/// `%APPDATA%\r5vlauncher\settings.json`
fn official_settings_path() -> Option<PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(PathBuf::from(appdata).join("r5vlauncher").join("settings.json"))
}

/// Platform default R5VLibrary base — mirrors the official launcher's logic.
fn default_library_base() -> Option<PathBuf> {
    let local = std::env::var("LOCALAPPDATA").ok()?;
    Some(Path::new(&local).join("Programs").join("R5VLibrary"))
}
