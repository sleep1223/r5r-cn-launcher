use crate::config::local_version::{normalize_community_version, read_build_version};
use crate::config::{Channel, UpdateStrategy};
use crate::dashboard::fetch_dashboard_config;
use crate::download::{run_install, InstallMode};
use crate::error::{AppError, AppResult};
use crate::events::{new_job_id, InstallJobId, InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use crate::manifest::fetch_manifest_for_version;
use crate::offline::dir_import::import_directory;
use crate::offline::shape_detect::detect_directory;
use crate::offline::zip_import::import_zip_strict;
use crate::offline::OfflineSource;
use crate::state::{JobHandle, LauncherState, PauseState};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

// ===== Offline import =====

#[tauri::command]
pub async fn start_offline_import(
    app: AppHandle,
    state: State<'_, LauncherState>,
    install_root: String,
    source: OfflineSource,
    _verify_after: bool,
) -> AppResult<InstallJobId> {
    if install_root.trim().is_empty() {
        return Err(AppError::settings("尚未配置安装根目录"));
    }
    if !install_root.is_ascii() {
        return Err(AppError::InvalidPath(
            "安装根目录不能包含中文或非 ASCII 字符".into(),
        ));
    }

    let install_root = PathBuf::from(install_root);
    let job_id = new_job_id();
    let cancel = CancellationToken::new();
    // Offline import doesn't honour pause (it's mostly local disk copying),
    // but JobHandle still needs a PauseState so cancel/remove behave uniformly.
    let pause = Arc::new(PauseState::new());

    state.jobs.insert(
        job_id.clone(),
        JobHandle {
            cancel: cancel.clone(),
            pause,
        },
    );

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    let jobs = state.jobs.clone();
    let settings_arc = state.settings.clone();
    let config_dir = state.config_dir.read().clone();

    tauri::async_runtime::spawn(async move {
        let result: AppResult<String> = async {
            match source {
                OfflineSource::Directory(p) => {
                    let shape = detect_directory(&PathBuf::from(p))?;
                    import_directory(
                        &app_clone,
                        &job_id_clone,
                        &shape,
                        &install_root,
                        cancel.clone(),
                    )
                    .await?;
                    Ok(shape.channel)
                }
                OfflineSource::Zip(p) => {
                    let zp = PathBuf::from(p);
                    // Strict path: require a readable EOCD / central
                    // directory. Truncated or malformed packs surface the
                    // friendly three-reason error up-front, so users know
                    // to re-download or merge split volumes instead of
                    // ending up with a half-written library.
                    let channel = import_zip_strict(
                        &app_clone,
                        &job_id_clone,
                        &zp,
                        &install_root,
                        cancel.clone(),
                    )
                    .await?;
                    Ok(channel)
                }
            }
        }
        .await;

        match result {
            Ok(channel) => {
                {
                    let mut s = settings_arc.write();
                    s.library_root = crate::util::display_slash(&install_root);
                    // Always point at the freshly-imported channel — the
                    // auto-select effect elsewhere may have set this to a
                    // different canonical value ("live_game") before the
                    // import ran, which left `install_dir` pointing at a
                    // folder that doesn't exist and caused the subsequent
                    // repair to re-download the entire pack.
                    s.selected_channel = channel.clone();
                    let entry = s.channels.entry(channel).or_default();
                    entry.installed = true;
                }
                let snapshot = settings_arc.read().clone();
                let _ = snapshot.save(&config_dir);
            }
            Err(AppError::Cancelled) => {
                let _ = app_clone.emit(
                    EVT_INSTALL_PROGRESS,
                    ProgressEvent::empty(job_id_clone.clone(), InstallPhase::Cancelled),
                );
            }
            Err(e) => {
                let _ = app_clone.emit(
                    EVT_INSTALL_PROGRESS,
                    ProgressEvent::empty(
                        job_id_clone.clone(),
                        InstallPhase::Failed {
                            reason: e.to_string(),
                        },
                    ),
                );
            }
        }

        jobs.remove(&job_id_clone);
    });

    Ok(job_id)
}

// ===== Online install / update / repair =====

#[tauri::command]
pub async fn start_online_install(
    app: AppHandle,
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<InstallJobId> {
    spawn_pipeline(app, state, channel, InstallMode::Install).await
}

#[tauri::command]
pub async fn start_update(
    app: AppHandle,
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<InstallJobId> {
    spawn_pipeline(app, state, channel, InstallMode::Update).await
}

#[tauri::command]
pub async fn start_repair(
    app: AppHandle,
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<InstallJobId> {
    spawn_pipeline(app, state, channel, InstallMode::Repair).await
}

#[tauri::command]
pub fn cancel_install(state: State<'_, LauncherState>, job_id: String) -> AppResult<bool> {
    Ok(state.jobs.cancel(&job_id))
}

/// Pause (or resume) a running install/update/repair. Offline imports ignore
/// the flag — they don't poll pause gates.
#[tauri::command]
pub fn pause_install(
    state: State<'_, LauncherState>,
    job_id: String,
    paused: bool,
) -> AppResult<bool> {
    Ok(state.jobs.set_paused(&job_id, paused))
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub has_update: bool,
    pub local_version: Option<String>,
    pub remote_version: String,
}

#[tauri::command]
pub async fn check_update(
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<UpdateStatus> {
    let (domain, dashboard_url, update_strategy, install_dir, saved_version) = {
        let settings = state.settings.read();
        (
            settings.mirror_domain.clone(),
            settings.dashboard_api_url.clone(),
            settings.update_strategy,
            settings.install_dir_for(&channel),
            settings
                .channels
                .get(&channel)
                .map(|entry| entry.version.clone())
                .filter(|version| !version.is_empty()),
        )
    };
    let ch = Channel::from_domain(&domain, &channel);
    let client = state.http.read().await.client();
    let community_version = if update_strategy == UpdateStrategy::Patch {
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
    let manifest = fetch_manifest_for_version(&client, &ch, community_version.as_deref()).await?;
    let manifest_version = manifest.game_version;
    let remote_version = if update_strategy == UpdateStrategy::Patch {
        community_version
            .as_deref()
            .and_then(|version| normalize_community_version(version, &manifest_version))
            .unwrap_or_else(|| manifest_version.clone())
    } else {
        manifest_version.clone()
    };
    if update_strategy == UpdateStrategy::Patch && remote_version != manifest_version {
        return Err(AppError::Manifest(format!(
            "社区版本 {} 与 checksums.json 版本 {} 不一致，请先同步目标清单",
            remote_version, manifest_version
        )));
    }
    let local_version = match install_dir.as_deref() {
        Some(dir) => read_build_version(dir, &remote_version).await,
        None => None,
    }
    .or(saved_version);
    tracing::info!(
        target: "update",
        channel = %channel,
        install_dir = ?install_dir,
        local_version = ?local_version,
        remote_version = %remote_version,
        "checked game version"
    );
    let has_update = match &local_version {
        Some(lv) => lv != &remote_version,
        None => true,
    };
    Ok(UpdateStatus {
        has_update,
        local_version,
        remote_version,
    })
}

// ===== shared spawner =====

async fn spawn_pipeline(
    app: AppHandle,
    state: State<'_, LauncherState>,
    channel: String,
    mode: InstallMode,
) -> AppResult<InstallJobId> {
    // Validate the install path before kicking off — fast-fail with a clear
    // error rather than emitting a Failed event ten seconds later.
    {
        let s = state.settings.read();
        if s.library_root.is_empty() {
            return Err(AppError::settings("尚未配置安装根目录"));
        }
        if !s.library_root.is_ascii() {
            return Err(AppError::InvalidPath(
                "安装根目录不能包含中文或非 ASCII 字符".into(),
            ));
        }
        if s.mirror_domain.is_empty() {
            return Err(AppError::settings("尚未配置镜像源域名"));
        }
    }

    let job_id = new_job_id();
    let cancel = CancellationToken::new();
    let pause = Arc::new(PauseState::new());
    state.jobs.insert(
        job_id.clone(),
        JobHandle {
            cancel: cancel.clone(),
            pause: pause.clone(),
        },
    );

    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    let jobs = state.jobs.clone();

    // We can't move `state: State<'_, LauncherState>` into the spawned task
    // because it borrows from the request scope. Pull out the Arcs we need.
    let settings_arc = state.settings.clone();
    let http_arc = state.http.clone();
    let config_dir_arc = state.config_dir.clone();

    tauri::async_runtime::spawn(async move {
        // Reconstruct a borrowed-shape LauncherState wrapper without the
        // tauri::State lifetime. Since LauncherState's fields are all Arc, we
        // just clone the Arcs into a fresh struct.
        let owned = crate::state::LauncherState {
            settings: settings_arc.clone(),
            http: http_arc.clone(),
            jobs: jobs.clone(),
            config_dir: config_dir_arc.clone(),
        };
        let _ = run_install(
            app_clone,
            &owned,
            job_id_clone.clone(),
            channel,
            mode,
            cancel,
            pause,
        )
        .await;
        jobs.remove(&job_id_clone);
    });

    Ok(job_id)
}
