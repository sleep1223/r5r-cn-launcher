use crate::config::Channel;
use crate::download::progress::ProgressAggregator;
use crate::download::retry::RetryPolicy;
use crate::error::{AppError, AppResult};
use crate::manifest::ManifestEntry;
use crate::state::PauseState;
use futures::StreamExt;
use reqwest::header::{CONTENT_RANGE, RANGE};
use reqwest::Client;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

/// Stream-download a single URL into `dest`. Reports each chunk to `agg`.
/// On cancellation, returns `AppError::Cancelled` (so retry doesn't try again).
///
/// Pauses honour `pause` at two points: before firing the request (so a pause
/// that started between retries holds the retry off) and between body chunks
/// (so an in-flight download freezes mid-stream). TCP backpressure keeps the
/// server quiet while we wait.
pub async fn stream_download(
    client: &Client,
    url: &str,
    channel: &Channel,
    dest: &Path,
    expected_size: u64,
    agg: &Arc<ProgressAggregator>,
    cancel: &CancellationToken,
    pause: &Arc<PauseState>,
    network_sem: &Arc<Semaphore>,
) -> AppResult<()> {
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    pause.wait().await;
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    let _network_permit = network_sem
        .acquire()
        .await
        .map_err(|e| AppError::other(format!("下载并发控制器已关闭: {}", e)))?;
    pause.wait().await;
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut resume_from = tokio::fs::metadata(dest)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if expected_size > 0 && resume_from >= expected_size {
        tokio::fs::File::create(dest).await?;
        resume_from = 0;
    }

    let req = client.get(url);
    let req = if resume_from > 0 {
        req.header(RANGE, format!("bytes={}-", resume_from))
    } else {
        req
    };
    let req = if channel.requires_key && !channel.key.is_empty() {
        req.header("channel-key", &channel.key)
    } else {
        req
    };
    // Timeout only covers connect + response headers (req.send() returns
    // once headers arrive). Body streaming runs without timeout, managed
    // by the cancel token, so large files are never cut short.
    let resp = tokio::time::timeout(Duration::from_secs(15), req.send())
        .await
        .map_err(|_| AppError::http(format!("GET {}: 服务器无响应 (15s)", url)))?
        .map_err(|e| AppError::http(format!("GET {}: {}", url, e)))?;
    if resume_from > 0 && resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        tokio::fs::File::create(dest).await?;
        return Err(AppError::http(format!("{} HTTP 416", url)));
    }
    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "{} HTTP {}",
            url,
            resp.status().as_u16()
        )));
    }

    let append = resume_from > 0 && resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if append {
        let expected_prefix = format!("bytes {}-", resume_from);
        let content_range = resp
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !content_range.starts_with(&expected_prefix) {
            tokio::fs::File::create(dest).await?;
            return Err(AppError::http(format!(
                "GET {}: Content-Range 无效 ({})",
                url, content_range
            )));
        }
    }
    let mut options = tokio::fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options.open(dest).await?;
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        if cancel.is_cancelled() {
            // Keep the partial file so a later retry can continue with Range.
            return Err(AppError::Cancelled);
        }
        let chunk = item.map_err(|e| AppError::http(format!("read body: {}", e)))?;
        file.write_all(&chunk).await?;
        agg.add_bytes(chunk.len() as u64);
        // Hold mid-stream if the user hit pause. We stop polling the bytes
        // stream, TCP backpressure throttles the sender. Long pauses can
        // eventually trip the server's idle timeout — if that happens the
        // retry policy will restart this file from scratch.
        if pause.is_paused() {
            pause.wait().await;
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
        }
    }
    file.flush().await?;
    file.sync_all().await?;
    if expected_size > 0 {
        let actual_size = tokio::fs::metadata(dest).await?.len();
        if actual_size != expected_size {
            return Err(AppError::Verification {
                path: url.to_string(),
                expected: expected_size.to_string(),
                actual: actual_size.to_string(),
            });
        }
    }
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

fn partial_path(dest: &Path) -> PathBuf {
    let mut value = OsString::from(dest.as_os_str());
    value.push(".r5r-part");
    PathBuf::from(value)
}

async fn replace_download(partial: &Path, dest: &Path) -> AppResult<()> {
    match tokio::fs::rename(partial, dest).await {
        Ok(()) => Ok(()),
        Err(first) if dest.exists() => {
            tokio::fs::remove_file(dest).await?;
            tokio::fs::rename(partial, dest).await.map_err(|second| {
                AppError::other(format!(
                    "替换下载文件失败: {}; 删除旧文件后重试仍失败: {}",
                    first, second
                ))
            })
        }
        Err(error) => Err(error.into()),
    }
}

async fn create_empty_file(dest: &Path) -> AppResult<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::File::create(dest).await?;
    Ok(())
}

/// Download a single non-chunked file with retry.
pub async fn download_single(
    client: &Client,
    channel: &Channel,
    entry: &ManifestEntry,
    install_dir: &Path,
    agg: &Arc<ProgressAggregator>,
    cancel: &CancellationToken,
    pause: &Arc<PauseState>,
    retry: &RetryPolicy,
    network_sem: &Arc<Semaphore>,
) -> AppResult<()> {
    let dest = entry_local_path(install_dir, &entry.path);
    let partial = partial_path(&dest);
    agg.set_current_file(&crate::util::normalize_slashes(&entry.path));

    // The official manifest can contain zero-byte placeholders that are not
    // stored as objects on the CDN. Materialize them locally instead of
    // requesting a URL that necessarily returns 404. File::create also
    // truncates an existing incorrect file during verify/update flows.
    if entry.size == 0 {
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        pause.wait().await;
        if cancel.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        create_empty_file(&dest).await?;
        agg.finish_file(&entry.path);
        return Ok(());
    }

    let url = entry_url(channel, &entry.path);
    retry
        .run(|_| {
            let url = url.clone();
            let partial = partial.clone();
            let pause = pause.clone();
            async move {
                stream_download(
                    client,
                    &url,
                    channel,
                    &partial,
                    entry.size,
                    agg,
                    cancel,
                    &pause,
                    network_sem,
                )
                .await?;
                if !entry.checksum.is_empty() {
                    let actual = crate::verify::sha256_file(&partial).await?;
                    if !actual.eq_ignore_ascii_case(&entry.checksum) {
                        let _ = tokio::fs::remove_file(&partial).await;
                        return Err(AppError::Verification {
                            path: entry.path.clone(),
                            expected: entry.checksum.clone(),
                            actual,
                        });
                    }
                }
                Ok(())
            }
        })
        .await?;
    pause.wait().await;
    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }
    replace_download(&partial, &dest).await?;
    agg.finish_file(&entry.path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create_empty_file, partial_path};
    use std::path::PathBuf;

    #[tokio::test]
    async fn creates_empty_file_and_missing_parent_directories() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("nested").join("placeholder.txt");

        create_empty_file(&target).await.unwrap();

        assert!(target.is_file());
        assert_eq!(tokio::fs::metadata(target).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn truncates_existing_file_to_zero_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("placeholder.txt");
        tokio::fs::write(&target, b"incorrect contents")
            .await
            .unwrap();

        create_empty_file(&target).await.unwrap();

        assert_eq!(tokio::fs::metadata(target).await.unwrap().len(), 0);
    }

    #[test]
    fn partial_download_is_a_stable_sibling_path() {
        let target = PathBuf::from(r"D:\Games\r5apex.exe");
        assert_eq!(
            partial_path(&target),
            PathBuf::from(r"D:\Games\r5apex.exe.r5r-part")
        );
    }
}
