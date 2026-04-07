use crate::config::fetch::{fetch_channel_version, fetch_remote_config};
use crate::config::{Channel, RemoteConfig};
use crate::download::chunk::download_chunked;
use crate::download::progress::ProgressAggregator;
use crate::download::retry::RetryPolicy;
use crate::download::worker::{download_single, entry_local_path};
use crate::error::{AppError, AppResult};
use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::manifest::{fetch_manifest, is_language_match, is_user_generated, ManifestEntry};
use crate::state::LauncherState;
use crate::verify::sha256_file;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// Fresh install: rewrite anything missing or wrong.
    Install,
    /// Refetch manifest, but only redownload mismatches. Short-circuit if the
    /// version is already up to date.
    Update,
    /// Same as Update but always proceeds (no version short-circuit).
    Repair,
}

/// Run an install/update/repair against the user's mirror.
///
/// Emits `install://progress` throughout. The aggregator emitter is owned by
/// this function and torn down on completion or cancellation.
pub async fn run_install(
    app: AppHandle,
    state: &LauncherState,
    job_id: String,
    channel_name: String,
    mode: InstallMode,
    cancel: CancellationToken,
) -> AppResult<()> {
    let emit = |phase: InstallPhase| {
        let _ = app.emit(
            EVT_INSTALL_PROGRESS,
            ProgressEvent::empty(job_id.clone(), phase),
        );
    };

    emit(InstallPhase::Preparing);

    // 1. Resolve config + channel.
    let (root_url, library_root, languages_wanted, concurrent_downloads) = {
        let s = state.settings.read();
        (
            s.root_config_url.clone(),
            s.library_root.clone(),
            vec!["schinese".to_string()],
            s.concurrent_downloads.max(1),
        )
    };
    if root_url.is_empty() {
        return Err(AppError::settings("尚未配置镜像 config.json 地址"));
    }
    if library_root.is_empty() {
        return Err(AppError::settings("尚未配置安装根目录"));
    }

    let client: Client = state.http.read().await.client();
    let cfg: RemoteConfig = fetch_remote_config(&client, &root_url).await?;
    let channel: Channel = cfg
        .channels
        .into_iter()
        .find(|c| c.name == channel_name)
        .ok_or_else(|| AppError::NotFound(format!("频道 {}", channel_name)))?;

    // 2. Resolve install dir.
    let install_dir = PathBuf::from(&library_root)
        .join("R5R Library")
        .join(channel.name.to_uppercase());
    tokio::fs::create_dir_all(&install_dir).await?;

    // 3. Version check (Update mode only).
    let remote_version = fetch_channel_version(&client, &channel).await.ok();
    if mode == InstallMode::Update {
        let local_version = state
            .settings
            .read()
            .channels
            .get(&channel.name)
            .map(|c| c.version.clone())
            .unwrap_or_default();
        if let Some(rv) = &remote_version {
            if !local_version.is_empty() && local_version == *rv {
                emit(InstallPhase::Complete);
                return Ok(());
            }
        }
    }

    // 4. Fetch manifest.
    let manifest = fetch_manifest(&client, &channel).await?;

    // 5. Build the download plan.
    let lang_refs: Vec<&str> = languages_wanted.iter().map(|s| s.as_str()).collect();
    let mut plan: Vec<ManifestEntry> = Vec::new();
    for entry in &manifest.files {
        if is_user_generated(&entry.path) {
            continue;
        }
        if entry.optional && !is_language_match(entry, &lang_refs) {
            continue;
        }
        let local = entry_local_path(&install_dir, &entry.path);
        let needs = if !local.exists() {
            true
        } else {
            // Compare local sha256 against manifest. Skip the hash check
            // entirely if the manifest has no checksum (defensive).
            if entry.checksum.is_empty() {
                false
            } else {
                let actual = sha256_file(&local).await.unwrap_or_default();
                !actual.eq_ignore_ascii_case(&entry.checksum)
            }
        };
        if needs {
            plan.push(entry.clone());
        }
    }

    if plan.is_empty() {
        // Nothing to do — but still bump version + installed flag.
        if let Some(rv) = &remote_version {
            let mut s = state.settings.write();
            let entry = s.channels.entry(channel.name.clone()).or_default();
            entry.version = rv.clone();
            entry.installed = true;
        }
        let _ = state.save_settings();
        emit(InstallPhase::Complete);
        return Ok(());
    }

    // 6. Execute downloads.
    let total_bytes: u64 = plan.iter().map(|e| e.size).sum();
    let agg = ProgressAggregator::new(job_id.clone(), plan.len(), total_bytes);
    let emitter_handle =
        agg.spawn_emitter(app.clone(), cancel.clone(), InstallPhase::Downloading);

    emit(InstallPhase::Downloading);

    let sem = Arc::new(Semaphore::new(concurrent_downloads as usize));
    let retry_full = RetryPolicy::full_file();
    let retry_chunk = RetryPolicy::chunk();

    let mut futs = FuturesUnordered::new();
    for entry in plan.iter().cloned() {
        let permit = sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::other(e.to_string()))?;
        let client = client.clone();
        let channel = channel.clone();
        let install_dir = install_dir.clone();
        let agg = agg.clone();
        let cancel = cancel.clone();
        futs.push(tokio::spawn(async move {
            let _permit = permit;
            if entry.parts.is_empty() {
                download_single(&client, &channel, &entry, &install_dir, &agg, &cancel, &retry_full)
                    .await
            } else {
                download_chunked(&client, &channel, &entry, &install_dir, &agg, &cancel, &retry_chunk)
                    .await
            }
        }));
    }

    let mut first_err: Option<AppError> = None;
    while let Some(joined) = futs.next().await {
        let r: AppResult<()> = joined.map_err(|e| AppError::other(e.to_string()))?;
        if let Err(e) = r {
            if first_err.is_none() {
                first_err = Some(e);
            }
            cancel.cancel(); // stop the rest fast
        }
    }
    emitter_handle.abort();

    if cancel.is_cancelled() && first_err.is_none() {
        emit(InstallPhase::Cancelled);
        return Err(AppError::Cancelled);
    }
    if let Some(e) = first_err {
        emit(InstallPhase::Failed {
            reason: e.to_string(),
        });
        return Err(e);
    }

    // 7. Verify pass.
    emit(InstallPhase::Verifying);
    for entry in &plan {
        if entry.checksum.is_empty() {
            continue;
        }
        let local = entry_local_path(&install_dir, &entry.path);
        let actual = sha256_file(&local).await?;
        if !actual.eq_ignore_ascii_case(&entry.checksum) {
            let err = AppError::Verification {
                path: entry.path.clone(),
                expected: entry.checksum.clone(),
                actual,
            };
            emit(InstallPhase::Failed {
                reason: err.to_string(),
            });
            return Err(err);
        }
    }

    // 8. Persist version + installed flag.
    {
        let mut s = state.settings.write();
        let ch_entry = s.channels.entry(channel.name.clone()).or_default();
        if let Some(rv) = &remote_version {
            ch_entry.version = rv.clone();
        }
        ch_entry.installed = true;
        if s.selected_channel.is_empty() {
            s.selected_channel = channel.name.clone();
        }
    }
    let _ = state.save_settings();

    emit(InstallPhase::Complete);
    Ok(())
}
