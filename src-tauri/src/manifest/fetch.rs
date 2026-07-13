use crate::config::Channel;
use crate::error::{AppError, AppResult};
use crate::manifest::GameManifest;
use reqwest::Client;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn fetch_manifest(client: &Client, channel: &Channel) -> AppResult<GameManifest> {
    fetch_manifest_for_version(client, channel, None).await
}

pub async fn fetch_manifest_for_version(
    client: &Client,
    channel: &Channel,
    version_hint: Option<&str>,
) -> AppResult<GameManifest> {
    let raw_url = format!("{}/checksums.json", channel.game_url.trim_end_matches('/'));
    let mut url = url::Url::parse(&raw_url)
        .map_err(|error| AppError::Manifest(format!("checksums.json URL 无效: {}", error)))?;
    if let Some(version) = version_hint.filter(|version| !version.trim().is_empty()) {
        url.query_pairs_mut().append_pair("version", version.trim());
    }
    let req = client.get(url).timeout(REQUEST_TIMEOUT);
    let req = if channel.requires_key && !channel.key.is_empty() {
        req.header("channel-key", &channel.key)
    } else {
        req
    };
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::http(format!("获取 checksums.json 失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "checksums.json 返回 HTTP {}",
            resp.status().as_u16()
        )));
    }
    let manifest: GameManifest = resp
        .json()
        .await
        .map_err(|e| AppError::Manifest(format!("解析 checksums.json 失败: {}", e)))?;
    Ok(manifest)
}
