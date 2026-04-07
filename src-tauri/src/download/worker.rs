use crate::config::Channel;
use crate::download::progress::ProgressAggregator;
use crate::download::retry::RetryPolicy;
use crate::error::{AppError, AppResult};
use crate::manifest::ManifestEntry;
use futures::StreamExt;
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

/// Stream-download a single URL into `dest`. Reports each chunk to `agg`.
/// On cancellation, returns `AppError::Cancelled` (so retry doesn't try again).
pub async fn stream_download(
    client: &Client,
    url: &str,
    channel: &Channel,
    dest: &Path,
    agg: &Arc<ProgressAggregator>,
    cancel: &CancellationToken,
) -> AppResult<()> {
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    let req = client.get(url);
    let req = if channel.requires_key && !channel.key.is_empty() {
        req.header("channel-key", &channel.key)
    } else {
        req
    };
    let resp = req
        .send()
        .await
        .map_err(|e| AppError::http(format!("GET {}: {}", url, e)))?;
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "{} HTTP {}",
            url,
            resp.status().as_u16()
        )));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        if cancel.is_cancelled() {
            // Best-effort cleanup; don't fail if remove fails.
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            return Err(AppError::Cancelled);
        }
        let chunk = item.map_err(|e| AppError::http(format!("read body: {}", e)))?;
        file.write_all(&chunk).await?;
        agg.add_bytes(chunk.len() as u64);
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok(())
}

/// Compute the absolute URL for a manifest entry: `{game_url}/{file.path}`,
/// normalizing backslashes to forward slashes (the manifest paths use Windows
/// separators but URLs need forward slashes).
pub fn entry_url(channel: &Channel, file_path: &str) -> String {
    format!(
        "{}/{}",
        channel.game_url.trim_end_matches('/'),
        file_path.replace('\\', "/")
    )
}

pub fn entry_local_path(install_dir: &Path, entry_path: &str) -> std::path::PathBuf {
    install_dir.join(entry_path.replace('\\', std::path::MAIN_SEPARATOR_STR))
}

/// Download a single non-chunked file with retry.
pub async fn download_single(
    client: &Client,
    channel: &Channel,
    entry: &ManifestEntry,
    install_dir: &Path,
    agg: &Arc<ProgressAggregator>,
    cancel: &CancellationToken,
    retry: &RetryPolicy,
) -> AppResult<()> {
    let url = entry_url(channel, &entry.path);
    let dest = entry_local_path(install_dir, &entry.path);
    agg.set_current_file(&entry.path);
    retry
        .run(|_| {
            let url = url.clone();
            let dest = dest.clone();
            async move { stream_download(client, &url, channel, &dest, agg, cancel).await }
        })
        .await?;
    agg.finish_file(&entry.path);
    Ok(())
}
