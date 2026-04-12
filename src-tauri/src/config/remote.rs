//! Channel definition for mirror-based downloads.
//! Since v0.7.0 we no longer fetch a `config.json` — the mirror domain plus
//! a channel name is enough to construct all URLs.

use serde::{Deserialize, Serialize};

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

/// The only channel we currently serve.
pub const DEFAULT_CHANNEL: &str = "live_game";

impl Channel {
    /// Build a `Channel` from a mirror domain and a channel name.
    /// URLs are `https://{domain}/launcher/{channel_name}/...`.
    pub fn from_domain(domain: &str, channel_name: &str) -> Self {
        let domain = domain.trim().trim_end_matches('/');
        Self {
            name: channel_name.to_string(),
            game_url: format!("https://{}/launcher/{}", domain, channel_name),
            dedi_url: String::new(),
            enabled: true,
            requires_key: false,
            allow_updates: true,
            key: String::new(),
        }
    }

    /// Folder name on disk for this channel — `R5R Library/<NAME_UPPERCASE>/`.
    pub fn folder_name(&self) -> String {
        self.name.to_uppercase()
    }
}
