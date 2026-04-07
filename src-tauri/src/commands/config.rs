use crate::config::fetch::{fetch_channel_version, fetch_remote_config};
use crate::config::RemoteConfig;
use crate::error::{AppError, AppResult};
use crate::state::LauncherState;
use tauri::State;

#[tauri::command]
pub async fn fetch_remote_config_cmd(
    state: State<'_, LauncherState>,
    url: Option<String>,
) -> AppResult<RemoteConfig> {
    let resolved = url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| state.settings.read().root_config_url.clone());
    let client = state.http.read().await.client();
    fetch_remote_config(&client, &resolved).await
}

#[tauri::command]
pub async fn get_channel_version(
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<String> {
    let resolved_url = state.settings.read().root_config_url.clone();
    let client = state.http.read().await.client();
    let cfg = fetch_remote_config(&client, &resolved_url).await?;
    let ch = cfg
        .channels
        .into_iter()
        .find(|c| c.name == channel)
        .ok_or_else(|| AppError::NotFound(format!("频道 {}", channel)))?;
    fetch_channel_version(&client, &ch).await
}
