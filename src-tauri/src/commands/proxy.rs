use crate::config::OFFICIAL_CONFIG_URL;
use crate::error::AppResult;
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

/// Test the current HTTP client (i.e. the active proxy mode) by issuing a HEAD
/// request against a config URL. The caller may pass `override_url` to test the
/// URL they're currently typing without having to save it first; otherwise we
/// fall back to the saved mirror URL, and if that's empty too we fall back to
/// the official R5R config URL — handy when the user is verifying that their
/// proxy can reach the official endpoint before configuring a mirror.
#[tauri::command]
pub async fn test_proxy(
    state: State<'_, LauncherState>,
    override_url: Option<String>,
) -> AppResult<ProxyTestResult> {
    let url = override_url
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let saved = state.settings.read().root_config_url.trim().to_string();
            if saved.is_empty() {
                None
            } else {
                Some(saved)
            }
        })
        .unwrap_or_else(|| OFFICIAL_CONFIG_URL.to_string());

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
