use crate::config::remote::channel_name_matches;
use crate::config::{Channel, RemoteConfig, OFFICIAL_DOMAIN};
use crate::error::{AppError, AppResult};
use reqwest::Client;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn fetch_remote_config(client: &Client) -> AppResult<RemoteConfig> {
    let url = format!("https://{}/launcher/config.json", OFFICIAL_DOMAIN);
    let resp = client
        .get(&url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| AppError::http(format!("获取官方 config.json 失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "官方 config.json 返回 HTTP {}",
            resp.status().as_u16()
        )));
    }
    resp.json()
        .await
        .map_err(|e| AppError::Manifest(format!("解析官方 config.json 失败: {}", e)))
}

pub async fn resolve_channel(
    client: &Client,
    requested_name: &str,
    mirror_domain: Option<&str>,
    key: String,
) -> AppResult<Channel> {
    let config = fetch_remote_config(client).await?;
    let remote = config
        .channels
        .iter()
        .find(|channel| channel_name_matches(&channel.name, requested_name))
        .ok_or_else(|| AppError::Manifest(format!("官方配置中不存在频道 {}", requested_name)))?;
    let mut channel = Channel::from_remote(remote, requested_name, mirror_domain, key);
    channel.allow_updates &= config.allow_updates;
    if !channel.enabled {
        return Err(AppError::Manifest(format!(
            "官方配置已禁用频道 {}",
            requested_name
        )));
    }
    if channel.requires_key && channel.key.trim().is_empty() {
        return Err(AppError::settings(format!(
            "频道 {} 需要访问密钥，请先在设置中配置",
            requested_name
        )));
    }
    Ok(channel)
}

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
