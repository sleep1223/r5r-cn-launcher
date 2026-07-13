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
pub const DEFAULT_CHANNEL: &str = "LIVE";

/// Map a local channel name (e.g. `"LIVE"` from auto-adopt) to the CDN path
/// segment. Unknown names are lowercased as-is.
fn cdn_path(channel_name: &str) -> String {
    match channel_name.to_uppercase().as_str() {
        "LIVE" => "live_game".to_string(),
        _ => channel_name.to_lowercase(),
    }
}

/// Canonical on-disk directory for a channel. `live_game` is the CDN/API
/// identifier for the regular LIVE channel, not a separate local directory.
pub fn channel_folder_name(channel_name: &str) -> String {
    match channel_name.to_uppercase().as_str() {
        "LIVE" | "LIVE_GAME" => "LIVE".to_string(),
        _ => channel_name.to_uppercase(),
    }
}

impl Channel {
    /// Build a `Channel` from a mirror domain and a channel name.
    /// The local `name` is kept as-is (for disk paths), but the CDN URL uses
    /// `cdn_path()` to map to the correct remote segment.
    pub fn from_domain(domain: &str, channel_name: &str) -> Self {
        let domain = domain.trim().trim_end_matches('/');
        let url_segment = cdn_path(channel_name);
        Self {
            name: channel_name.to_string(),
            game_url: format!("https://{}/launcher/{}", domain, url_segment),
            dedi_url: String::new(),
            enabled: true,
            requires_key: false,
            allow_updates: true,
            key: String::new(),
        }
    }

    /// Folder name on disk for this channel — `R5R Library/<NAME_UPPERCASE>/`.
    pub fn folder_name(&self) -> String {
        channel_folder_name(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_api_name_uses_the_live_disk_folder() {
        assert_eq!(channel_folder_name("live_game"), "LIVE");
        assert_eq!(channel_folder_name("LIVE"), "LIVE");
    }
}
