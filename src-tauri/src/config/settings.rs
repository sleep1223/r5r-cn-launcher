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
/// correct. `Patch` (TODO) applies binary patches from the dashboard's
/// `patches[]`, falling back to `Verify` when no patch path covers the gap.
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

    #[serde(default = "default_root_config_url")]
    pub root_config_url: String,

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

    /// How "更新" resolves outdated files (校验 vs. 补丁包). Defaults to
    /// `Verify` because patches aren't wired through the pipeline yet.
    #[serde(default)]
    pub update_strategy: UpdateStrategy,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA,
            proxy_mode: ProxyMode::default(),
            root_config_url: default_root_config_url(),
            library_root: String::new(),
            selected_channel: String::new(),
            concurrent_downloads: default_concurrency(),
            channels: HashMap::new(),
            launch_option_selection: serde_json::Value::Null,
            last_known_official_install_path: None,
            dashboard_api_url: default_dashboard_api_url(),
            update_strategy: UpdateStrategy::default(),
        }
    }
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
/// Default mirror config URL. Points at the community CN mirror so users can
/// install out-of-the-box without first hunting down a working URL. Users
/// behind a different mirror are still free to override this in settings.
pub const DEFAULT_MIRROR_CONFIG_URL: &str = "https://cdn-r5r-org.sleep0.de/launcher/config.json";

/// Official R5Reloaded config URL — used as a connectivity-test target when
/// the user hasn't filled in any mirror URL yet.
pub const OFFICIAL_CONFIG_URL: &str = "https://cdn.r5r.org/launcher/config.json";

fn default_root_config_url() -> String {
    DEFAULT_MIRROR_CONFIG_URL.to_string()
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
        // Future schema migrations live here. For now we just stamp the current
        // version on any file that didn't have one.
        if self.schema_version == 0 {
            self.schema_version = CURRENT_SCHEMA;
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
