pub mod accelerator;
pub mod commands;
pub mod config;
pub mod dashboard;
pub mod detect;
pub mod download;
pub mod error;
pub mod events;
pub mod launch_options;
pub mod manifest;
pub mod offline;
pub mod process;
pub mod proxy;
pub mod state;
pub mod updater;
pub mod verify;

use crate::config::LauncherSettings;
use crate::proxy::{HttpClientFactory, ProxyMode};
use crate::state::LauncherState;
use parking_lot::Mutex;
use tauri::Manager;
use tracing_appender::non_blocking::WorkerGuard;

pub const USER_AGENT: &str = concat!("R5R-Launcher-CN/", env!("CARGO_PKG_VERSION"));

/// Holds the tracing-appender worker guard for the app's lifetime — dropping
/// this would flush and close the log file, so we stash it in app state.
struct LogGuard(#[allow(dead_code)] Mutex<Option<WorkerGuard>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Resolve config + log dirs.
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("无法获取应用配置目录");
            let log_dir = app
                .path()
                .app_log_dir()
                .unwrap_or_else(|_| config_dir.join("logs"));
            std::fs::create_dir_all(&config_dir).ok();
            std::fs::create_dir_all(&log_dir).ok();

            // Daily-rolling file log + stderr in dev. The WorkerGuard MUST be
            // kept alive for the program's lifetime.
            let file_appender =
                tracing_appender::rolling::daily(&log_dir, "launcher.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .with_writer(non_blocking)
                .with_ansi(false)
                .try_init();
            app.manage(LogGuard(Mutex::new(Some(guard))));
            tracing::info!(target: "launcher", "R5R-CN launcher v{} starting", env!("CARGO_PKG_VERSION"));

            // Load settings (or default).
            let settings = LauncherSettings::load_or_default(&config_dir)
                .unwrap_or_else(|e| {
                    tracing::warn!("加载 settings.json 失败，使用默认值: {}", e);
                    LauncherSettings::default()
                });

            // Build initial HTTP client from the persisted proxy mode. Failure
            // here falls back to a direct (no-proxy) client so the launcher
            // still starts; the user can fix proxy settings from the UI.
            let http = HttpClientFactory::new(settings.proxy_mode.clone(), USER_AGENT)
                .or_else(|_| HttpClientFactory::new(ProxyMode::None, USER_AGENT))
                .expect("HTTP 客户端构建彻底失败");

            let state = LauncherState::new(settings, http);
            *state.config_dir.write() = config_dir;
            app.manage(state);

            // Background accelerator poller. Scans the running process list
            // every 15s; if the set of detected accelerators changes (new
            // one started, or one closed) we emit `accelerator://changed`
            // so the UI can re-render its warning banner without having to
            // poll from the frontend. The first scan runs immediately
            // (`tokio::time::interval`'s first `tick()` is instant).
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use std::time::Duration;
                    use tauri::Emitter;
                    let mut last_signature: Vec<String> = Vec::new();
                    let mut interval = tokio::time::interval(Duration::from_secs(15));
                    loop {
                        interval.tick().await;
                        let found = crate::accelerator::detect();
                        let mut signature: Vec<String> =
                            found.iter().map(|d| d.name.clone()).collect();
                        signature.sort();
                        if signature != last_signature {
                            last_signature = signature;
                            let _ = handle.emit(
                                crate::events::EVT_ACCELERATOR_CHANGED,
                                &found,
                            );
                            tracing::info!(
                                target: "accelerator",
                                "detected accelerators changed: {:?}",
                                found.iter().map(|d| &d.name).collect::<Vec<_>>()
                            );
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::settings::validate_install_path,
            commands::settings::open_log_folder,
            commands::settings::open_external_url,
            commands::accelerator::detect_accelerators_cmd,
            commands::proxy::set_proxy_mode,
            commands::proxy::test_proxy,
            commands::detect::detect_existing_r5r,
            commands::detect::auto_adopt_existing_install,
            commands::config::fetch_remote_config_cmd,
            commands::config::get_channel_version,
            commands::dashboard::fetch_dashboard_config_cmd,
            commands::launch_options::get_launch_option_catalog,
            commands::launch_options::validate_launch_args_cmd,
            commands::launch_options::compose_launch_args_cmd,
            commands::launch::launch_game_cmd,
            commands::install::start_offline_import,
            commands::install::start_online_install,
            commands::install::start_update,
            commands::install::start_repair,
            commands::install::cancel_install,
            commands::install::check_update,
            commands::updater::get_launcher_version,
            commands::updater::download_and_apply_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
