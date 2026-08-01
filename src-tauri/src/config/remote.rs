//! Official remote launcher configuration and resolved channel definitions.

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConfig {
    #[serde(default)]
    pub launcher_version: String,
    #[serde(default)]
    pub updater_version: String,
    #[serde(default)]
    pub force_updates: bool,
    #[serde(default = "default_true")]
    pub allow_updates: bool,
    #[serde(default)]
    pub self_updater: String,
    #[serde(default)]
    pub background_video: String,
    #[serde(default)]
    pub channels: Vec<Channel>,
}

fn default_true() -> bool {
    true
}

/// The only channel we currently serve.
pub const DEFAULT_CHANNEL: &str = "LIVE";

/// Canonical on-disk directory for a channel. `live_game` is the CDN/API
/// identifier for the regular LIVE channel, not a separate local directory.
pub fn channel_folder_name(channel_name: &str) -> String {
    match channel_name.to_uppercase().as_str() {
        "LIVE" | "LIVE_GAME" => "LIVE".to_string(),
        _ => channel_name.to_uppercase(),
    }
}

impl Channel {
    /// Folder name on disk for this channel — `R5R Library/<NAME_UPPERCASE>/`.
    pub fn folder_name(&self) -> String {
        channel_folder_name(&self.name)
    }

    pub fn from_remote(
        remote: &Channel,
        requested_name: &str,
        override_domain: Option<&str>,
        key: String,
    ) -> Self {
        let game_url = override_domain
            .filter(|domain| !domain.trim().is_empty())
            .and_then(|domain| replace_url_host(&remote.game_url, domain))
            .unwrap_or_else(|| remote.game_url.clone());
        Self {
            name: requested_name.to_string(),
            game_url,
            dedi_url: remote.dedi_url.clone(),
            enabled: remote.enabled,
            requires_key: remote.requires_key,
            allow_updates: remote.allow_updates,
            key,
        }
    }
}

fn replace_url_host(source: &str, domain: &str) -> Option<String> {
    let mut url = url::Url::parse(source).ok()?;
    let domain = domain.trim().trim_end_matches('/');
    let override_url = if domain.starts_with("http://") || domain.starts_with("https://") {
        url::Url::parse(domain).ok()?
    } else {
        url::Url::parse(&format!("https://{}", domain)).ok()?
    };
    url.set_host(override_url.host_str()).ok()?;
    url.set_port(override_url.port()).ok()?;
    url.set_scheme(override_url.scheme()).ok()?;
    Some(url.to_string().trim_end_matches('/').to_string())
}

pub fn channel_name_matches(config_name: &str, requested_name: &str) -> bool {
    config_name.eq_ignore_ascii_case(requested_name)
        || (requested_name.eq_ignore_ascii_case("LIVE") && config_name.eq_ignore_ascii_case("live"))
        || (requested_name.eq_ignore_ascii_case("live_game")
            && config_name.eq_ignore_ascii_case("live"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_api_name_uses_the_live_disk_folder() {
        assert_eq!(channel_folder_name("live_game"), "LIVE");
        assert_eq!(channel_folder_name("LIVE"), "LIVE");
    }

    #[test]
    fn remote_channel_keeps_flags_when_using_a_mirror_host() {
        let remote = Channel {
            name: "live".into(),
            game_url: "https://cdn.r5r.org/launcher/live_game".into(),
            enabled: false,
            requires_key: true,
            allow_updates: false,
            ..Default::default()
        };
        let channel = Channel::from_remote(
            &remote,
            "LIVE",
            Some("mirror.example:8443"),
            "secret".into(),
        );
        assert_eq!(
            channel.game_url,
            "https://mirror.example:8443/launcher/live_game"
        );
        assert!(!channel.enabled);
        assert!(channel.requires_key);
        assert!(!channel.allow_updates);
        assert_eq!(channel.key, "secret");
    }

    #[test]
    fn deserializes_official_remote_config_shape() {
        let config: RemoteConfig = serde_json::from_str(
            r#"{
                "launcherVersion":"1.6.3",
                "allowUpdates":false,
                "channels":[{
                    "name":"live",
                    "game_url":"https://cdn.r5r.org/launcher/live_game",
                    "enabled":true,
                    "requires_key":true,
                    "allow_updates":false
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(config.launcher_version, "1.6.3");
        assert!(!config.allow_updates);
        assert_eq!(config.channels.len(), 1);
        assert!(config.channels[0].requires_key);
        assert!(!config.channels[0].allow_updates);
    }
}
