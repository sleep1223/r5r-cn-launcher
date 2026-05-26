use crate::dashboard::DEFAULT_DASHBOARD_API_URL;
use crate::error::{AppError, AppResult};
use crate::proxy::ProxyMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "settings.json";
pub const CURRENT_SCHEMA: u32 = 1;

/// How updates resolve mismatched files. `Verify` walks the manifest and
/// re-downloads anything whose SHA-256 doesn't match — slow but always
/// correct. `Patch` applies a dashboard-provided zip from `patches[]`, verifies
/// the patched install against the target manifest, then falls back to `Verify`
/// when no patch path covers the gap or the patch cannot be accepted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStrategy {
    #[default]
    Verify,
    Patch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSettings {
    #[serde(default = "default_schema")]
    pub schema_version: u32,

    #[serde(default)]
    pub proxy_mode: ProxyMode,

    /// Low-traffic metadata domain (e.g. `cdn-r5r-org.sleep0.de`). Used for
    /// `checksums.json`, `version.txt`, and the connectivity probe — URLs are
    /// `https://{mirror_domain}/launcher/{channel}/...`. Actual game files are
    /// served from `dashboard.download_domain` instead; this domain is only
    /// the fallback when the dashboard is unreachable or doesn't supply one.
    #[serde(default = "default_mirror_domain", alias = "root_config_url")]
    pub mirror_domain: String,

    /// Where the user wants `R5R Library/` to live. The actual install dir is
    /// `library_root/R5R Library/<CHANNEL>/`.
    #[serde(default)]
    pub library_root: String,

    #[serde(default)]
    pub selected_channel: String,

    #[serde(default = "default_concurrency")]
    pub concurrent_downloads: u32,

    /// Per-channel state: installed flag, local version, key override, etc.
    #[serde(default)]
    pub channels: HashMap<String, PerChannelState>,

    /// Persisted launch-option selection (see `launch_options::model`).
    #[serde(default)]
    pub launch_option_selection: serde_json::Value,

    /// First detected official R5R install path, shown as a hint when the
    /// user is picking a new install location.
    #[serde(default)]
    pub last_known_official_install_path: Option<String>,

    /// Community dashboard endpoint (announcement / rules / patch metadata).
    #[serde(default = "default_dashboard_api_url")]
    pub dashboard_api_url: String,

    /// How "更新" resolves outdated files (校验 vs. 补丁包).
    #[serde(default)]
    pub update_strategy: UpdateStrategy,

    /// Whether to download the optional HD texture packs (`*.opt.starpak`,
    /// manifest entries with `optional: true`). Off by default to match the
    /// official launcher, which only touches HD textures when the user has
    /// already installed them.
    #[serde(default)]
    pub download_hd_textures: bool,

    /// Whether to start EA App before launching the game and wait briefly for
    /// its process to appear. Best-effort: a 5s timeout is non-fatal, but if
    /// EA App can't be started at all (not installed) we abort the launch.
    #[serde(default = "default_launch_via_ea_app")]
    pub launch_via_ea_app: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA,
            proxy_mode: ProxyMode::default(),
            mirror_domain: default_mirror_domain(),
            library_root: String::new(),
            selected_channel: String::new(),
            concurrent_downloads: default_concurrency(),
            channels: HashMap::new(),
            launch_option_selection: serde_json::Value::Null,
            last_known_official_install_path: None,
            dashboard_api_url: default_dashboard_api_url(),
            update_strategy: UpdateStrategy::default(),
            download_hd_textures: false,
            launch_via_ea_app: default_launch_via_ea_app(),
        }
    }
}

fn default_launch_via_ea_app() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerChannelState {
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub installed_languages: Vec<String>,
}

fn default_schema() -> u32 {
    CURRENT_SCHEMA
}
/// Default mirror CDN domain. Users behind a different mirror can override.
pub const DEFAULT_MIRROR_DOMAIN: &str = "cdn-r5r-org.sleep0.de";

/// Official R5Reloaded CDN domain — used as a connectivity-test fallback.
pub const OFFICIAL_DOMAIN: &str = "cdn.r5r.org";

fn default_mirror_domain() -> String {
    DEFAULT_MIRROR_DOMAIN.to_string()
}
fn default_concurrency() -> u32 {
    4
}
fn default_dashboard_api_url() -> String {
    DEFAULT_DASHBOARD_API_URL.to_string()
}

impl LauncherSettings {
    pub fn load_or_default(dir: &Path) -> AppResult<Self> {
        let path = dir.join(SETTINGS_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(&path)?;
        let mut s: Self = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::settings(format!("解析 settings.json 失败: {}", e)))?;
        s.migrate();
        Ok(s)
    }

    pub fn save(&self, dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(SETTINGS_FILE);
        let bytes = serde_json::to_vec_pretty(self)?;
        // Atomic-ish: write to a sibling temp file then rename.
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    fn migrate(&mut self) {
        if self.schema_version == 0 {
            self.schema_version = CURRENT_SCHEMA;
        }
        // v1→v2: root_config_url (full URL) was replaced by mirror_domain.
        // serde(alias) already deserializes the old key into mirror_domain,
        // but the value might still be a full URL — strip it down to just the host.
        if self.mirror_domain.starts_with("http://") || self.mirror_domain.starts_with("https://") {
            if let Ok(url) = url::Url::parse(&self.mirror_domain) {
                if let Some(host) = url.host_str() {
                    self.mirror_domain = host.to_string();
                }
            }
        }
    }

    pub fn install_dir_for(&self, channel_name: &str) -> Option<PathBuf> {
        if self.library_root.is_empty() {
            return None;
        }
        Some(
            PathBuf::from(&self.library_root)
                .join("R5R Library")
                .join(channel_name.to_uppercase()),
        )
    }
}
