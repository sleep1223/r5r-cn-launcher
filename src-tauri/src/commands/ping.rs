use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

/// Measure TCP-connect latency to `host:port`. We use TCP rather than ICMP
/// because ICMP requires privileged raw sockets on most platforms, and game
/// servers reliably accept connections on their advertised port anyway.
#[tauri::command]
pub async fn ping_host(host: String, port: u16, timeout_ms: Option<u64>) -> AppResult<PingResult> {
    let host = host.trim().to_string();
    if host.is_empty() || port == 0 {
        return Ok(PingResult {
            ok: false,
            latency_ms: None,
            error: Some("invalid host or port".into()),
        });
    }
    let dur = Duration::from_millis(timeout_ms.unwrap_or(2000));
    let addr = format!("{}:{}", host, port);
    let started = Instant::now();
    match timeout(dur, TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => Ok(PingResult {
            ok: true,
            latency_ms: Some(started.elapsed().as_millis() as u64),
            error: None,
        }),
        Ok(Err(e)) => Ok(PingResult {
            ok: false,
            latency_ms: None,
            error: Some(e.to_string()),
        }),
        Err(_) => Ok(PingResult {
            ok: false,
            latency_ms: None,
            error: Some("timeout".into()),
        }),
    }
}
