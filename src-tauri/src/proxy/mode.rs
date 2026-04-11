use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "url", rename_all = "snake_case")]
pub enum ProxyMode {
    /// Use the OS-level proxy (Windows internet settings, macOS scutil, Linux env).
    System,
    /// Explicit proxy URL like `http://127.0.0.1:7890` or `socks5://127.0.0.1:1080`.
    Custom(String),
    /// Direct connection — no proxy at all.
    None,
}

impl Default for ProxyMode {
    fn default() -> Self {
        // We default to direct connection because the whole point of this
        // launcher is a Chinese mirror — system proxies (VPN/加速器) would
        // route the mirror traffic through the wrong exit and defeat it.
        ProxyMode::None
    }
}
