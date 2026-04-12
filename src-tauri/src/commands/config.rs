use crate::config::fetch::fetch_channel_version;
use crate::config::Channel;
use crate::error::AppResult;
use crate::state::LauncherState;
use tauri::State;

#[tauri::command]
pub async fn get_channel_version(
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<String> {
    let domain = state.settings.read().mirror_domain.clone();
    let ch = Channel::from_domain(&domain, &channel);
    let client = state.http.read().await.client();
    fetch_channel_version(&client, &ch).await
}
