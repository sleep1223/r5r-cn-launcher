use crate::error::AppResult;
use crate::state::LauncherState;
use crate::updater;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub current: String,
}

/// Return the current launcher version so the frontend can compare it with
/// the dashboard's `launcher_version` without hardcoding.
#[tauri::command]
pub fn get_launcher_version() -> VersionInfo {
    VersionInfo {
        current: updater::CURRENT_VERSION.to_string(),
    }
}

/// Download the update installer from `url`, run it silently, and exit.
/// The frontend should call this only after confirming with the user (or
/// auto-calling it when `force_update` is set).
///
/// Emits `update://progress` events during the download phase.
#[tauri::command]
pub async fn download_and_apply_update(
    app: AppHandle,
    state: State<'_, LauncherState>,
    url: String,
) -> AppResult<()> {
    if url.is_empty() {
        return Err(crate::error::AppError::other(
            "更新链接为空，无法执行自动更新",
        ));
    }
    let client = state.http.read().await.client();
    let path = updater::download_installer(&app, &client, &url).await?;
    updater::run_installer_and_exit(&path)?;
    Ok(())
}
