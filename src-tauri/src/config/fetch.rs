use crate::config::Channel;
use crate::error::{AppError, AppResult};
use reqwest::Client;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// `GET {channel.game_url}/version.txt` — returns the version string with
/// surrounding whitespace trimmed.
pub async fn fetch_channel_version(client: &Client, channel: &Channel) -> AppResult<String> {
    let url = format!("{}/version.txt", channel.game_url.trim_end_matches('/'));
    let req = client.get(&url).timeout(REQUEST_TIMEOUT);
    let req = if channel.requires_key && !channel.key.is_empty() {
        req.header("channel-key", &channel.key)
    } else {
        req
    };
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::http(format!("获取 version.txt 失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "version.txt 返回 HTTP {}",
            resp.status().as_u16()
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| AppError::http(format!("读取 version.txt 失败: {}", e)))?;
    Ok(text.trim().to_string())
}
