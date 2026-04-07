use crate::config::Channel;
use crate::error::{AppError, AppResult};
use crate::manifest::GameManifest;
use reqwest::Client;

pub async fn fetch_manifest(client: &Client, channel: &Channel) -> AppResult<GameManifest> {
    let url = format!("{}/checksums.json", channel.game_url.trim_end_matches('/'));
    let req = client.get(&url);
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
