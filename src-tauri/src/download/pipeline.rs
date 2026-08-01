use crate::config::fetch::resolve_channel;
use crate::config::local_version::{normalize_community_version, read_build_version};
use crate::config::{Channel, UpdateStrategy, OFFICIAL_DOMAIN};
use crate::dashboard::fetch_dashboard_config;
use crate::download::chunk::download_chunked;
use crate::download::patch::{apply_patch, PatchOutcome};
use crate::download::progress::ProgressAggregator;
use crate::download::retry::RetryPolicy;
use crate::download::worker::{download_single, entry_local_path};
use crate::error::{AppError, AppResult};
use crate::events::{
    InstallLogEvent, InstallPhase, LogLevel, ProgressEvent, EVT_INSTALL_LOG, EVT_INSTALL_PROGRESS,
};
use crate::manifest::{
    fetch_manifest_for_version, is_language_match, is_user_generated, ManifestEntry,
};
use crate::state::{LauncherState, PauseState};
use crate::verify::sha256_file;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

const MAX_SCAN_CONCURRENCY: u32 = 8;

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
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!("开始安装频道 {}", channel_name),
    );

    // 1. Resolve the low-traffic metadata channel (`checksums.json` +
    //    `version.txt`) from the configured mirror, or the official CDN when
    //    no mirror is configured.
    let (
        mirror_domain,
        dashboard_url,
        library_root,
        languages_wanted,
        concurrent_downloads,
        update_strategy,
        download_hd_textures,
    ) = {
        let s = state.settings.read();
        (
            s.mirror_domain.clone(),
            s.dashboard_api_url.clone(),
            s.library_root.clone(),
            s.installed_languages_for(&channel_name),
            s.normalized_download_concurrency(),
            s.update_strategy,
            s.download_hd_textures,
        )
    };
    if library_root.is_empty() {
        return Err(AppError::settings("尚未配置安装根目录"));
    }

    let mirror_configured = !mirror_domain.trim().is_empty();
    let metadata_domain = if mirror_configured {
        mirror_domain.trim()
    } else {
        OFFICIAL_DOMAIN
    };
    let client: Client = state.http.read().await.client();
    let channel_key = state.settings.read().channel_key_for(&channel_name);
    let channel = resolve_channel(
        &client,
        &channel_name,
        mirror_configured.then_some(metadata_domain),
        channel_key,
    )
    .await?;
    if mode == InstallMode::Update && !channel.allow_updates {
        return Err(AppError::Manifest(format!(
            "官方配置当前不允许更新频道 {}",
            channel_name
        )));
    }
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!(
            "频道 {} (元数据 game_url={})",
            channel.name, channel.game_url
        ),
    );
    // In patch mode the dashboard's community version is authoritative. Use
    // it as a stable cache key for checksums.json so a newly published release
    // does not reuse the previous version's CDN object.
    let is_patch_update = mode == InstallMode::Update && update_strategy == UpdateStrategy::Patch;
    let community_version = if is_patch_update {
        let version = fetch_dashboard_config(&client, &dashboard_url)
            .await?
            .game_version;
        if version.trim().is_empty() {
            return Err(AppError::Manifest("社区服版本号为空".into()));
        }
        Some(version)
    } else {
        None
    };

    // 2. Resolve install dir.
    let install_dir = state
        .settings
        .read()
        .install_dir_for(&channel_name)
        .ok_or_else(|| AppError::settings("尚未配置安装根目录"))?;
    tokio::fs::create_dir_all(&install_dir).await?;
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!("安装目录: {}", crate::util::display_slash(&install_dir)),
    );

    // 3. Fetch manifest (version comes from checksums.json's game_version).
    emit(InstallPhase::FetchingManifest);
    emit_log(&app, &job_id, LogLevel::Info, "拉取游戏 checksums.json …");
    let manifest = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
            emit(InstallPhase::Cancelled);
            return Err(AppError::Cancelled);
        }
        r = fetch_manifest_for_version(&client, &channel, community_version.as_deref()) => r?,
    };
    let manifest_version = manifest.game_version.clone();
    let remote_version = if manifest_version.is_empty() {
        None
    } else if is_patch_update {
        Some(
            community_version
                .as_deref()
                .and_then(|version| normalize_community_version(version, &manifest_version))
                .unwrap_or_else(|| manifest_version.clone()),
        )
    } else {
        Some(manifest_version.clone())
    };
    if let Some(remote) = remote_version.as_deref() {
        if is_patch_update && remote != manifest_version {
            return Err(AppError::Manifest(format!(
                "社区版本 {} 与 checksums.json 版本 {} 不一致，请先同步目标清单",
                remote, manifest_version
            )));
        }
    }
    emit_log(
        &app,
        &job_id,
        LogLevel::Info,
        format!(
            "manifest 共 {} 个文件，版本: {}",
            manifest.files.len(),
            remote_version.as_deref().unwrap_or("未知")
        ),
    );

    // 4. Version check (Update mode only).
    if mode == InstallMode::Update {
        let saved_version = state
            .settings
            .read()
            .channels
            .get(&channel.name)
            .map(|c| c.version.clone())
            .unwrap_or_default();
        let local_version = match remote_version.as_deref() {
            Some(rv) => read_build_version(&install_dir, rv).await,
            None => None,
        }
        .unwrap_or(saved_version);
        if let Some(rv) = &remote_version {
            if !local_version.is_empty() && local_version == *rv {
                emit_log(
                    &app,
                    &job_id,
                    LogLevel::Info,
                    "本地版本与远端一致，无需更新",
                );
                emit(InstallPhase::Complete);
                return Ok(());
            }
            // Patch strategy: try to download + extract a patch zip that
            // covers exactly `local_version → remote_version`. On success
            // we're done; if the dashboard has no matching patch, we log and
            // fall through to the full verify pipeline below.
            if is_patch_update && !local_version.is_empty() {
                emit_log(
                    &app,
                    &job_id,
                    LogLevel::Info,
                    format!("尝试应用补丁包: {} → {}", local_version, rv),
                );
                match apply_patch(
                    &app,
                    state,
                    &job_id,
                    &channel.name,
                    &local_version,
                    rv,
                    &manifest,
                    cancel.clone(),
                )
                .await
                {
                    Ok(PatchOutcome::Applied) => {
                        emit_log(&app, &job_id, LogLevel::Info, "补丁包应用完成 ✓");
                        return Ok(());
                    }
                    Ok(PatchOutcome::NotApplicable) => {
                        emit_log(
                            &app,
                            &job_id,
                            LogLevel::Warn,
                            "未找到匹配的补丁包路径，回退到完整校验",
                        );
                    }
                    Err(AppError::Cancelled) => {
                        emit(InstallPhase::Cancelled);
                        return Err(AppError::Cancelled);
                    }
                    Err(e) => {
                        emit_log(
                            &app,
                            &job_id,
                            LogLevel::Warn,
                            format!("补丁包应用失败: {} — 回退到完整校验", e),
                        );
                    }
                }
            }
        }
    }

    // 5. Build the download plan.
    //
    // Walk every manifest entry, filter out user-generated and unwanted
    // languages, then for the rest verify the on-disk file's SHA-256 in
    // parallel. Already-correct files get skipped — that's the resume path
    // when the user hits "安装" after a partially-completed previous run.
    emit(InstallPhase::Scanning);
    emit_log(&app, &job_id, LogLevel::Info, "校验已下载文件中 …");

    // Mirror the official launcher's tri-split manifest handling
    // (`ApiService.GetGameManifestAsync(optional)` + `GetLanguageFilesAsync`):
    //   - core    : non-optional, empty language
    //   - language: non-empty language — keep only the ones the user wants
    //   - optional: the `*.opt.starpak` HD texture pack, gated behind the
    //     user's `download_hd_textures` setting
    let lang_refs: Vec<&str> = languages_wanted.iter().map(|s| s.as_str()).collect();
    let candidates: Vec<ManifestEntry> = manifest
        .files
        .iter()
        .filter(|entry| !is_user_generated(&entry.path))
        .filter(|entry| {
            if entry.optional {
                download_hd_textures && entry.language.is_empty()
            } else if entry.language.is_empty() {
                true
            } else {
                is_language_match(entry, &lang_refs)
            }
        })
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

    // Hash existing files concurrently, but keep disk work capped separately
    // from the network setting so high download concurrency does not thrash
    // slow drives.
    let scan_concurrency = concurrent_downloads.min(MAX_SCAN_CONCURRENCY);
    let scan_sem = Arc::new(Semaphore::new(scan_concurrency as usize));
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
    //
    // A configured metadata mirror opts into the dashboard-managed game-file
    // domain. Without a configured mirror, the official CDN is the primary
    // source. A dashboard failure in mirror mode is fatal and never falls
    // through to another domain.
    let download_channel = if mirror_configured {
        match resolve_download_channel(&app, &job_id, &client, &dashboard_url, &channel).await {
            Ok(channel) => channel,
            Err(e) => {
                emit_log(
                    &app,
                    &job_id,
                    LogLevel::Error,
                    format!("获取游戏下载域名失败: {}", e),
                );
                emit(InstallPhase::Failed {
                    reason: e.to_string(),
                });
                return Err(e);
            }
        }
    } else {
        let channel = channel.clone();
        emit_log(
            &app,
            &job_id,
            LogLevel::Info,
            format!("未设置镜像源，使用官方游戏下载源: {}", channel.game_url),
        );
        channel
    };

    let total_bytes: u64 = plan.iter().map(|e| e.size).sum();
    let agg = ProgressAggregator::new(job_id.clone(), plan.len(), total_bytes);
    let emitter_handle = agg.spawn_emitter(app.clone(), cancel.clone(), InstallPhase::Downloading);

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

    let retry_full = RetryPolicy::full_file();
    let retry_chunk = RetryPolicy::chunk();
    let network_sem = Arc::new(Semaphore::new(concurrent_downloads as usize));

    let spawn_download = |tasks: &mut JoinSet<AppResult<()>>, entry: ManifestEntry| {
        let client = client.clone();
        let download_channel = download_channel.clone();
        let install_dir = install_dir.clone();
        let agg = agg.clone();
        let cancel = cancel.clone();
        let pause = pause.clone();
        let network_sem = network_sem.clone();
        tasks.spawn(async move {
            pause.wait().await;
            if cancel.is_cancelled() {
                return Err(AppError::Cancelled);
            }
            if entry.parts.is_empty() {
                download_single(
                    &client,
                    &download_channel,
                    &entry,
                    &install_dir,
                    &agg,
                    &cancel,
                    &pause,
                    &retry_full,
                    &network_sem,
                )
                .await
            } else {
                download_chunked(
                    &client,
                    &download_channel,
                    &entry,
                    &install_dir,
                    &agg,
                    &cancel,
                    &pause,
                    &retry_chunk,
                    &network_sem,
                )
                .await
            }
        });
    };

    // Keep only `concurrent_downloads` tasks alive at once and replenish the
    // window as each task finishes. Polling completions while dispatching is
    // important: the previous semaphore loop queued every manifest entry
    // before it observed the first HTTP error, which could cycle through
    // thousands of failed small-file requests while progress remained at 0 B.
    let mut entries = plan.iter().cloned();
    let mut tasks = JoinSet::new();
    for _ in 0..concurrent_downloads {
        let Some(entry) = entries.next() else {
            break;
        };
        spawn_download(&mut tasks, entry);
    }

    let mut first_err: Option<AppError> = None;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(Ok(())) => {
                if !cancel.is_cancelled() && first_err.is_none() {
                    if let Some(entry) = entries.next() {
                        spawn_download(&mut tasks, entry);
                    }
                }
            }
            Ok(Err(AppError::Cancelled)) => {}
            Ok(Err(e)) => {
                if first_err.is_none() {
                    first_err = Some(e);
                    cancel.cancel();
                }
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(AppError::other(e.to_string()));
                    cancel.cancel();
                }
            }
        }
    }
    emitter_handle.abort();

    if cancel.is_cancelled() && first_err.is_none() {
        emit_log(&app, &job_id, LogLevel::Warn, "用户取消安装");
        emit(InstallPhase::Cancelled);
        return Err(AppError::Cancelled);
    }
    if let Some(e) = first_err {
        emit_log(&app, &job_id, LogLevel::Error, format!("下载失败: {}", e));
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
            emit_log(&app, &job_id, LogLevel::Error, format!("校验失败: {}", err));
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

/// Build the game-file channel strictly from the dashboard API's
/// `download_domain`. No normalization, probing, or fallback is applied.
async fn resolve_download_channel(
    app: &AppHandle,
    job_id: &str,
    client: &Client,
    dashboard_url: &str,
    metadata_channel: &Channel,
) -> AppResult<Channel> {
    let dashboard = fetch_dashboard_config(client, dashboard_url).await?;
    let domain = dashboard.download_domain.trim();
    if domain.is_empty() {
        return Err(AppError::http("数据面板响应中的 download_domain 为空"));
    }

    let channel = Channel::from_remote(
        metadata_channel,
        &metadata_channel.name,
        Some(domain),
        metadata_channel.key.clone(),
    );
    emit_log(
        app,
        job_id,
        LogLevel::Info,
        format!(
            "游戏下载源 (dashboard.download_domain={}): {}",
            domain, channel.game_url
        ),
    );
    Ok(channel)
}
