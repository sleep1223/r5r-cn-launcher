//! Community dashboard API — fetches launcher metadata, announcements, rules,
//! and patch info from the CN community endpoint at
//! `https://r5.sleep0.de/api/v1/r5/launcher/config`.
//!
//! This is a separate channel from the official R5R `RemoteConfig` (which is
//! the wire-compatible mirror config). The dashboard surface only powers the
//! Home tab UI — it does not drive any download behavior.

use crate::error::{AppError, AppResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub const DEFAULT_DASHBOARD_API_URL: &str = "https://r5.sleep0.de/api/v1/r5/launcher/config";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DashboardConfig {
    #[serde(default)]
    pub offline_package_url: String,
    #[serde(default)]
    pub docs_url: String,
    #[serde(default)]
    pub launcher_version: String,
    #[serde(default)]
    pub launcher_update_url: String,
    #[serde(default)]
    pub force_update: bool,
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub patches: Vec<PatchEntry>,
    #[serde(default)]
    pub announcement: Announcement,
    #[serde(default)]
    pub rules: Vec<RuleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatchEntry {
    #[serde(default)]
    pub from_version: String,
    #[serde(default)]
    pub to_version: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Announcement {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleEntry {
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiEnvelope {
    #[serde(default)]
    code: String,
    data: Option<DashboardConfig>,
    #[serde(default)]
    msg: String,
}

/// Fetch + unwrap the dashboard config envelope. The API uses a `{code, data, msg}`
/// shape and returns `code = "0000"` on success.
pub async fn fetch_dashboard_config(client: &Client, url: &str) -> AppResult<DashboardConfig> {
    if url.is_empty() {
        return Err(AppError::settings("尚未配置数据面板 API 地址"));
    }
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::http(format!("请求数据面板失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "数据面板返回 HTTP {}",
            resp.status().as_u16()
        )));
    }
    let envelope: ApiEnvelope = resp
        .json()
        .await
        .map_err(|e| AppError::http(format!("解析数据面板响应失败: {}", e)))?;
    if envelope.code != "0000" {
        return Err(AppError::http(format!(
            "数据面板错误 code={} msg={}",
            envelope.code, envelope.msg
        )));
    }
    envelope
        .data
        .ok_or_else(|| AppError::http("数据面板响应缺少 data 字段".to_string()))
}
