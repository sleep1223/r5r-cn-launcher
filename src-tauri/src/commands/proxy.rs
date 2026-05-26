use crate::config::OFFICIAL_DOMAIN;
use crate::error::AppResult;
use crate::events::EVT_PROXY_CHANGED;
use crate::proxy::ProxyMode;
use crate::state::LauncherState;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
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

/// Test the current HTTP client by issuing a HEAD request against the mirror.
/// The caller may pass `override_domain` to test the domain they're currently
/// typing without having to save it first; otherwise we fall back to the saved
/// mirror domain, and if that's empty too we fall back to the official CDN.
#[tauri::command]
pub async fn test_proxy(
    state: State<'_, LauncherState>,
    override_domain: Option<String>,
) -> AppResult<ProxyTestResult> {
    let domain = override_domain
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let saved = state.settings.read().mirror_domain.trim().to_string();
            if saved.is_empty() {
                None
            } else {
                Some(saved)
            }
        })
        .unwrap_or_else(|| OFFICIAL_DOMAIN.to_string());
    let url = format!(
        "https://{}/launcher/live_game/checksums.json",
        domain.trim_end_matches('/')
    );

    let client = state.http.read().await.client();
    let started = Instant::now();
    let res = client
        .head(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await;
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
