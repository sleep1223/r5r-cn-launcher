use crate::error::{AppError, AppResult};
use crate::events::EVT_PROXY_CHANGED;
use crate::proxy::ProxyMode;
use crate::state::LauncherState;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn set_proxy_mode(
    app: AppHandle,
    state: State<'_, LauncherState>,
    mode: ProxyMode,
) -> AppResult<()> {
    {
        let mut http = state.http.write().await;
        http.rebuild(mode.clone())?;
    }
    state.settings.write().proxy_mode = mode;
    state.save_settings()?;
    let _ = app.emit(EVT_PROXY_CHANGED, ());
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTestResult {
    pub ok: bool,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn test_proxy(state: State<'_, LauncherState>) -> AppResult<ProxyTestResult> {
    let url = state.settings.read().root_config_url.clone();
    if url.is_empty() {
        return Err(AppError::settings("尚未配置镜像 config.json 地址"));
    }
    let client = state.http.read().await.client();
    let started = Instant::now();
    let res = client.head(&url).send().await;
    let latency_ms = started.elapsed().as_millis() as u64;
    match res {
        Ok(r) => Ok(ProxyTestResult {
            ok: r.status().is_success(),
            status: Some(r.status().as_u16()),
            latency_ms,
            error: None,
        }),
        Err(e) => Ok(ProxyTestResult {
            ok: false,
            status: None,
            latency_ms,
            error: Some(e.to_string()),
        }),
    }
}
