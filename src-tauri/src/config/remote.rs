//! Wire-compatible mirror of the official R5Reloaded `RemoteConfig` schema.
//! The official launcher emits camelCase JSON keys; we use serde renames so
//! Rust-side code can use idiomatic snake_case while still being able to
//! consume an unmodified `config.json` from the official CDN.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteConfig {
    #[serde(default, rename = "launcherVersion")]
    pub launcher_version: String,
    #[serde(default, rename = "updaterVersion")]
    pub updater_version: String,
    #[serde(default, rename = "selfUpdater")]
    pub self_updater: String,
    #[serde(default, rename = "backgroundVideo")]
    pub background_video: String,
    #[serde(default, rename = "allowUpdates")]
    pub allow_updates: bool,
    #[serde(default, rename = "forceUpdates")]
    pub force_updates: bool,
    #[serde(default)]
    pub channels: Vec<Channel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Channel {
    pub name: String,
    pub game_url: String,
    #[serde(default)]
    pub dedi_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub requires_key: bool,
    #[serde(default = "default_true")]
    pub allow_updates: bool,
    /// Channel-key header value, when `requires_key`. Empty otherwise.
    #[serde(default)]
    pub key: String,
}

fn default_true() -> bool {
    true
}

impl Channel {
    /// Folder name on disk for this channel — `R5R Library/<NAME_UPPERCASE>/`.
    pub fn folder_name(&self) -> String {
        self.name.to_uppercase()
    }
}
