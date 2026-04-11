use crate::config::fetch::{fetch_channel_version, fetch_remote_config};
use crate::config::{Channel, RemoteConfig, UpdateStrategy};
use crate::download::chunk::download_chunked;
use crate::download::progress::ProgressAggregator;
use crate::download::retry::RetryPolicy;
use crate::download::worker::{download_single, entry_local_path};
use crate::error::{AppError, AppResult};
use crate::events::{
    InstallLogEvent, InstallPhase, LogLevel, ProgressEvent, EVT_INSTALL_LOG,
    EVT_INSTALL_PROGRESS,
};
use crate::manifest::{fetch_manifest, is_language_match, is_user_generated, ManifestEntry};
use crate::state::{LauncherState, PauseState};
use crate::verify::sha256_file;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::Client;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn emit_log(app: &AppHandle, job_id: &str, level: LogLevel, message: impl Into<String>) {
    let msg = message.into();
    tracing::info!(target: "install", job = %job_id, "{}", msg);
    let _ = app.emit(
        EVT_INSTALL_LOG,
        InstallLogEvent {
            job_id: job_id.to_string(),
            ts_ms: now_ms(),
            level,
            message: msg,
        },
    );
}

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
    pause: Arc<PauseState>,
) -> AppResult<()> {
    let emit = |phase: InstallPhase| {
        let _ = app.emit(
            EVT_INSTALL_PROGRESS,
            ProgressEvent::empty(job_id.clone(), phase),
        );
    };

    emit(InstallPhase::Preparing);
    emit_log(&app, &job_id, LogLevel::Info, format!("开始安装频道 {}", channel_name));

    // 1. Resolve config + channel.
    let (root_url, library_root, languages_wanted, concurrent_downloads, update_strategy) = {
        let s = state.settings.read();
        (
            s.root_config_url.clone(),
            s.library_root.clone(),
            vec!["schinese".to_string()],
            s.concurrent_downloads.max(1),
            s.update_strategy,
        )
    };
    if mode == InstallMode::Update && update_strategy == UpdateStrategy::Patch {
        emit_log(
            &app,
            &job_id,
            LogLevel::Warn,
            "当前更新策略为「补丁包」，但补丁路径暂未实现，回退到完整校验",
        );
    }
    if root_url.is_empty() {
        return Err(AppError::settings("尚未配置镜像 config.json 地址"));
    }
    if library_root.is_empty() {
        return Err(AppError::settings("尚未配置安装根目录"));
    }

    let client: Client = state.http.read().await.client();
    emit(InstallPhase::FetchingConfig);
    emit_log(&app, &job_id, LogLevel::Info, format!("拉取镜像 config.json: {}", root_url));
    // Wrap the HTTP fetches in `tokio::select!` against the cancel token so
    // clicking 取消 immediately unblocks even when reqwest is hung waiting on a
    // slow/dead mirror. Without this, the user would stare at "拉取 config"
    // until reqwest's 300s read timeout fired.
    let cfg: RemoteConfig = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
        r = fetch_remote_config(&client, &root_url) => r?,
    };
    let channel: Channel = cfg
        .channels
        .into_iter()
        .find(|c| c.name == channel_name)
        .ok_or_else(|| AppError::NotFound(format!("频道 {}", channel_name)))?;
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!("解析到频道 {} (game_url={})", channel.name, channel.game_url),
    );

    // 2. Resolve install dir.
    let install_dir = PathBuf::from(&library_root)
        .join("R5R Library")
        .join(channel.name.to_uppercase());
    tokio::fs::create_dir_all(&install_dir).await?;
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!("安装目录: {}", install_dir.display()),
    );

    // 3. Version check (Update mode only).
    let remote_version = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
        r = fetch_channel_version(&client, &channel) => r.ok(),
    };
    if let Some(rv) = &remote_version {
        emit_log(&app, &job_id, LogLevel::Info, format!("远端版本: {}", rv));
    }
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
                emit_log(&app, &job_id, LogLevel::Info, "本地版本与远端一致，无需更新");
                emit(InstallPhase::Complete);
                return Ok(());
            }
        }
    }

    // 4. Fetch manifest.
    emit(InstallPhase::FetchingManifest);
    emit_log(&app, &job_id, LogLevel::Info, "拉取游戏 checksums.json …");
    let manifest = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
        r = fetch_manifest(&client, &channel) => r?,
    };
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!("manifest 共 {} 个文件", manifest.files.len()),
    );

    // 5. Build the download plan.
    //
    // Walk every manifest entry, filter out user-generated and unwanted
    // languages, then for the rest verify the on-disk file's SHA-256 in
    // parallel. Already-correct files get skipped — that's the resume path
    // when the user hits "安装" after a partially-completed previous run.
    emit(InstallPhase::Scanning);
    emit_log(&app, &job_id, LogLevel::Info, "校验已下载文件中 …");

    let lang_refs: Vec<&str> = languages_wanted.iter().map(|s| s.as_str()).collect();
    let candidates: Vec<ManifestEntry> = manifest
        .files
        .iter()
        .filter(|entry| !is_user_generated(&entry.path))
        .filter(|entry| !entry.optional || is_language_match(entry, &lang_refs))
        .cloned()
        .collect();

    let scan_total = candidates.len();
    let scan_done = Arc::new(AtomicUsize::new(0));
    let scan_skipped = Arc::new(AtomicUsize::new(0));

    // Periodically push a progress event so the UI can show "已校验 N/M" while
    // scanning runs.
    let scan_emitter = {
        let app = app.clone();
        let job_id = job_id.clone();
        let scan_done = scan_done.clone();
        let scan_total_v = scan_total;
        let cancel = cancel.clone();
        tauri::async_runtime::spawn(async move {
            let mut t = tokio::time::interval(std::time::Duration::from_millis(200));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = t.tick() => {
                        let done = scan_done.load(Ordering::Relaxed);
                        let _ = app.emit(
                            EVT_INSTALL_PROGRESS,
                            ProgressEvent {
                                job_id: job_id.clone(),
                                phase: InstallPhase::Scanning,
                                file_index: done,
                                file_count: scan_total_v,
                                bytes_done: 0,
                                bytes_total: 0,
                                current_file: String::new(),
                                speed_bps: 0,
                                eta_seconds: 0,
                            },
                        );
                        if done >= scan_total_v {
                            break;
                        }
                    }
                }
            }
        })
    };

    // Hash existing files concurrently — bound by concurrent_downloads so we
    // don't thrash the disk on slow drives.
    let scan_sem = Arc::new(Semaphore::new(concurrent_downloads as usize));
    let scan_results: AppResult<Vec<Option<ManifestEntry>>> = async {
        let mut futs = FuturesUnordered::new();
        for entry in candidates.iter().cloned() {
            // Bail out of the spawn loop fast on cancel — don't queue up more
            // hashing work we'll just throw away.
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            // Hold the spawn loop while paused so we don't burn permits.
            pause.wait().await;
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            let permit = scan_sem
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| AppError::other(e.to_string()))?;
            let install_dir = install_dir.clone();
            let scan_done = scan_done.clone();
            let scan_skipped = scan_skipped.clone();
            let pause = pause.clone();
            let cancel = cancel.clone();
            futs.push(tokio::spawn(async move {
                let _permit = permit;
                if cancel.is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                pause.wait().await;
                if cancel.is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                let local = entry_local_path(&install_dir, &entry.path);
                let needs = if !local.exists() {
                    true
                } else if entry.checksum.is_empty() {
                    // Defensive: if the manifest has no checksum, trust the
                    // on-disk file as-is (matches old behavior).
                    false
                } else {
                    let actual = sha256_file(&local).await.unwrap_or_default();
                    !actual.eq_ignore_ascii_case(&entry.checksum)
                };
                scan_done.fetch_add(1, Ordering::Relaxed);
                if !needs {
                    scan_skipped.fetch_add(1, Ordering::Relaxed);
                    Ok(None)
                } else {
                    Ok(Some(entry))
                }
            }));
        }
        let mut out = Vec::with_capacity(scan_total);
        while let Some(joined) = futs.next().await {
            let r: AppResult<Option<ManifestEntry>> =
                joined.map_err(|e| AppError::other(e.to_string()))?;
            out.push(r?);
        }
        Ok(out)
    }
    .await;
    scan_emitter.abort();
    let scan_results = match scan_results {
        Ok(v) => v,
        Err(AppError::Cancelled) => {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
        Err(e) => return Err(e),
    };

    let plan: Vec<ManifestEntry> = scan_results.into_iter().flatten().collect();
    let skipped = scan_skipped.load(Ordering::Relaxed);
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!(
            "校验完成: 跳过已下载 {} 个 / 待下载 {} 个 / 总 {} 个",
            skipped,
            plan.len(),
            scan_total
        ),
    );

    if plan.is_empty() {
        emit_log(&app, &job_id, LogLevel::Info, "无文件需要下载，安装完成");
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
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!(
            "开始下载 {} 个文件，共 {} 字节，并发 {}",
            plan.len(),
            total_bytes,
            concurrent_downloads
        ),
    );

    let sem = Arc::new(Semaphore::new(concurrent_downloads as usize));
    let retry_full = RetryPolicy::full_file();
    let retry_chunk = RetryPolicy::chunk();

    let mut futs = FuturesUnordered::new();
    for entry in plan.iter().cloned() {
        if cancel.is_cancelled() {
            break;
        }
        // Pause the outer dispatch loop — no point burning semaphore permits
        // while the user wants the pipeline frozen.
        pause.wait().await;
        if cancel.is_cancelled() {
            break;
        }
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
        let pause = pause.clone();
        futs.push(tokio::spawn(async move {
            let _permit = permit;
            if entry.parts.is_empty() {
                download_single(
                    &client,
                    &channel,
                    &entry,
                    &install_dir,
                    &agg,
                    &cancel,
                    &pause,
                    &retry_full,
                )
                .await
            } else {
                download_chunked(
                    &client,
                    &channel,
                    &entry,
                    &install_dir,
                    &agg,
                    &cancel,
                    &pause,
                    &retry_chunk,
                )
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
        emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
        emit(InstallPhase::Cancelled);
        return Err(AppError::Cancelled);
    }
    if let Some(e) = first_err {
        emit_log(
            &app,
            &job_id,
            LogLevel::Error,
            format!("下载失败: {}", e),
        );
        emit(InstallPhase::Failed {
            reason: e.to_string(),
        });
        return Err(e);
    }

    // 7. Verify pass.
    emit(InstallPhase::Verifying);
    emit_log(&app, &job_id, LogLevel::Info, "校验下载结果 …");
    for entry in &plan {
        if cancel.is_cancelled() {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
        pause.wait().await;
        if cancel.is_cancelled() {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
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
            emit_log(
                &app,
                &job_id,
                LogLevel::Error,
                format!("校验失败: {}", err),
            );
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

    emit_log(&app, &job_id, LogLevel::Info, "安装完成 ✓");
    emit(InstallPhase::Complete);
    Ok(())
}
