use crate::error::{AppError, AppResult};
use crate::proxy::ProxyMode;
use reqwest::{Client, Proxy};
use std::time::Duration;

/// Owns the current `reqwest::Client` plus the proxy mode it was built from.
/// `rebuild` swaps the cached client; in-flight requests holding a clone of
/// the previous client are not affected, only new requests use the new one.
pub struct HttpClientFactory {
    mode: ProxyMode,
    client: Client,
    user_agent: String,
}

impl HttpClientFactory {
    pub fn new(mode: ProxyMode, user_agent: impl Into<String>) -> AppResult<Self> {
        let ua = user_agent.into();
        let client = build_client(&mode, &ua)?;
        Ok(Self {
            mode,
            client,
            user_agent: ua,
        })
    }

    pub fn rebuild(&mut self, mode: ProxyMode) -> AppResult<()> {
        let client = build_client(&mode, &self.user_agent)?;
        self.client = client;
        self.mode = mode;
        Ok(())
    }

    pub fn client(&self) -> Client {
        self.client.clone()
    }

    pub fn mode(&self) -> &ProxyMode {
        &self.mode
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}

fn build_client(mode: &ProxyMode, user_agent: &str) -> AppResult<Client> {
    let mut builder = Client::builder()
        .user_agent(user_agent)
        .timeout(Duration::from_secs(300))
        .connect_timeout(Duration::from_secs(15))
        .pool_idle_timeout(Some(Duration::from_secs(30)));

    match mode {
        ProxyMode::None => {
            builder = builder.no_proxy();
        }
        ProxyMode::Custom(url) => {
            let proxy = Proxy::all(url)
                .map_err(|e| AppError::http(format!("代理 URL 无效: {}", e)))?;
            builder = builder.proxy(proxy);
        }
        ProxyMode::System => {
            // Read the system proxy explicitly. We don't trust reqwest's
            // env-var auto-detection because Chinese users frequently have a
            // system proxy set in Windows/macOS settings without setting any
            // *_PROXY env var.
            if let Some(url) = detect_system_proxy() {
                if let Ok(proxy) = Proxy::all(&url) {
                    builder = builder.proxy(proxy);
                }
            }
        }
    }

    builder
        .build()
        .map_err(|e| AppError::http(format!("HTTP 客户端构建失败: {}", e)))
}

fn detect_system_proxy() -> Option<String> {
    use sysproxy::Sysproxy;
    let sp = Sysproxy::get_system_proxy().ok()?;
    if !sp.enable {
        return None;
    }
    // Prefer HTTP form even when the user has a SOCKS proxy configured;
    // most Chinese clients (clash, v2ray) expose both on the same host.
    Some(format!("http://{}:{}", sp.host, sp.port))
}
