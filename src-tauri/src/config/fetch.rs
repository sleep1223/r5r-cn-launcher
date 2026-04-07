use crate::config::{Channel, RemoteConfig};
use crate::error::{AppError, AppResult};
use reqwest::Client;

/// Fetch the launcher root config from the user's mirror.
pub async fn fetch_remote_config(client: &Client, url: &str) -> AppResult<RemoteConfig> {
    if url.is_empty() {
        return Err(AppError::settings("尚未配置镜像 config.json 地址"));
    }
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::http(format!("获取 config.json 失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "镜像返回 HTTP {}",
            resp.status().as_u16()
        )));
    }
    let cfg: RemoteConfig = resp
        .json()
        .await
        .map_err(|e| AppError::http(format!("解析 config.json 失败: {}", e)))?;
    Ok(cfg)
}

/// `GET {channel.game_url}/version.txt` — returns the version string with
/// surrounding whitespace trimmed.
pub async fn fetch_channel_version(client: &Client, channel: &Channel) -> AppResult<String> {
    let url = format!("{}/version.txt", channel.game_url.trim_end_matches('/'));
    let req = client.get(&url);
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
