use crate::dashboard::{fetch_dashboard_config, DashboardConfig};
use crate::error::AppResult;
use crate::state::LauncherState;
use tauri::State;

#[tauri::command]
pub async fn fetch_dashboard_config_cmd(
    state: State<'_, LauncherState>,
    url: Option<String>,
) -> AppResult<DashboardConfig> {
    // The override is only used by the settings tab's "测试一下" affordance —
    // normal callers leave it empty and we read from settings.
    let resolved = url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| state.settings.read().dashboard_api_url.clone());
    let client = state.http.read().await.client();
    fetch_dashboard_config(&client, &resolved).await
}
