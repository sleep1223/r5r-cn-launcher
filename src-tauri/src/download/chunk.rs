use crate::config::Channel;
use crate::download::progress::ProgressAggregator;
use crate::download::retry::RetryPolicy;
use crate::download::worker::{entry_local_path, entry_url, stream_download};
use crate::error::{AppError, AppResult};
use crate::manifest::ManifestEntry;
use crate::state::PauseState;
use crate::verify::sha256_file;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

const MAX_PARTS_PER_FILE: usize = 8;

/// Download all chunks of a multi-part file in parallel, then merge them
/// sequentially into the final destination and clean up the temp parts.
pub async fn download_chunked(
    client: &Client,
    channel: &Channel,
    entry: &ManifestEntry,
    install_dir: &Path,
    agg: &Arc<ProgressAggregator>,
    cancel: &CancellationToken,
    pause: &Arc<PauseState>,
    retry: &RetryPolicy,
) -> AppResult<()> {
    let dest = entry_local_path(install_dir, &entry.path);
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmpdir = tempfile::tempdir_in(install_dir)
        .map_err(|e| AppError::other(format!("tempdir: {}", e)))?;
    let tmp_root = tmpdir.path().to_path_buf();

    agg.set_current_file(&entry.path);

    // ===== Parallel chunk download =====
    let chunk_sem = Arc::new(Semaphore::new(MAX_PARTS_PER_FILE));
    let mut futs = FuturesUnordered::new();
    for (idx, part) in entry.parts.iter().enumerate() {
        let permit = chunk_sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::other(e.to_string()))?;
        let part = part.clone();
        let client = client.clone();
        let channel_clone = channel.clone();
        let agg = agg.clone();
        let cancel = cancel.clone();
        let pause_outer = pause.clone();
        let tmp_root = tmp_root.clone();
        let retry = *retry;
        futs.push(tokio::spawn(async move {
            let _permit = permit;
            // Local part filename — use the index so merge order is deterministic.
            let part_dest = tmp_root.join(format!("part_{:06}.bin", idx));
            let url = entry_url(&channel_clone, &part.path);
            retry
                .run(|_| {
                    let url = url.clone();
                    let part_dest = part_dest.clone();
                    let client = client.clone();
                    let channel = channel_clone.clone();
                    let agg = agg.clone();
                    let cancel = cancel.clone();
                    let pause = pause_outer.clone();
                    async move {
                        stream_download(
                            &client, &url, &channel, &part_dest, &agg, &cancel, &pause,
                        )
                        .await
                    }
                })
                .await?;
            // Verify chunk hash before accepting it.
            let actual = sha256_file(&part_dest).await?;
            if !actual.eq_ignore_ascii_case(&part.checksum) && !part.checksum.is_empty() {
                return Err(AppError::Verification {
                    path: part.path.clone(),
                    expected: part.checksum.clone(),
                    actual,
                });
            }
            Ok::<(usize, std::path::PathBuf), AppError>((idx, part_dest))
        }));
    }

    let mut parts_in_order: Vec<Option<std::path::PathBuf>> = vec![None; entry.parts.len()];
    while let Some(joined) = futs.next().await {
        let (idx, path) = joined.map_err(|e| AppError::other(e.to_string()))??;
        parts_in_order[idx] = Some(path);
    }

    if cancel.is_cancelled() {
        return Err(AppError::Cancelled);
    }

    // ===== Merge =====
    use tokio::io::AsyncWriteExt;
    let mut out = tokio::fs::File::create(&dest).await?;
    for slot in &parts_in_order {
        let p = slot
            .as_ref()
            .ok_or_else(|| AppError::other("missing chunk after download"))?;
        let mut input = tokio::fs::File::open(p).await?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            use tokio::io::AsyncReadExt;
            let n = input.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).await?;
        }
    }
    out.flush().await?;
    out.sync_all().await?;

    // tmpdir auto-cleans on Drop.
    agg.finish_file(&entry.path);
    Ok(())
}
