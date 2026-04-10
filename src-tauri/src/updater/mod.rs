//! Launcher self-update.
//!
//! Flow:
//! 1. Frontend fetches the community dashboard → gets `launcher_version`,
//!    `launcher_update_url`, `force_update`.
//! 2. Compares `launcher_version` against `env!("CARGO_PKG_VERSION")`.
//! 3. If newer + URL non-empty → calls `download_and_apply_update`.
//! 4. Backend downloads the NSIS installer to a temp dir, runs it with `/S`
//!    (silent) then exits the current process so the installer can replace
//!    the files.
//!
//! This avoids needing Tauri's `tauri-plugin-updater` key infrastructure —
//! the installer is signed by whatever certificate the release CI uses.

use crate::error::{AppError, AppResult};
use futures::StreamExt;
use reqwest::Client;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const EVT_UPDATE_PROGRESS: &str = "update://progress";

#[derive(Debug, Clone, Serialize)]
pub struct UpdateProgress {
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub phase: UpdatePhase,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Downloading,
    Installing,
    Failed { reason: String },
}

/// Download the installer from `url` to a temp file, emitting progress events.
/// Returns the path to the downloaded file.
pub async fn download_installer(
    app: &AppHandle,
    client: &Client,
    url: &str,
) -> AppResult<PathBuf> {
    tracing::info!(target: "updater", "downloading installer from {}", url);

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::http(format!("下载更新失败: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::http(format!(
            "下载更新返回 HTTP {}",
            resp.status().as_u16()
        )));
    }

    let total = resp.content_length();
    let tmp_dir = std::env::temp_dir().join("r5r-cn-launcher-update");
    std::fs::create_dir_all(&tmp_dir)?;

    // Derive filename from URL or use a generic name.
    let filename = url
        .rsplit('/')
        .next()
        .filter(|s| s.ends_with(".exe") || s.ends_with(".msi"))
        .unwrap_or("R5R-CN-Launcher-setup.exe");
    let dest = tmp_dir.join(filename);

    let mut file = tokio::fs::File::create(&dest).await?;
    let mut stream = resp.bytes_stream();
    let mut done: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::http(format!("读取更新流失败: {}", e)))?;
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;

        // Emit progress at most every 200ms.
        if last_emit.elapsed() >= std::time::Duration::from_millis(200) {
            let _ = app.emit(
                EVT_UPDATE_PROGRESS,
                UpdateProgress {
                    bytes_done: done,
                    bytes_total: total,
                    phase: UpdatePhase::Downloading,
                },
            );
            last_emit = std::time::Instant::now();
        }
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    // Final progress event.
    let _ = app.emit(
        EVT_UPDATE_PROGRESS,
        UpdateProgress {
            bytes_done: done,
            bytes_total: total,
            phase: UpdatePhase::Installing,
        },
    );

    tracing::info!(target: "updater", "downloaded {} bytes to {}", done, dest.display());
    Ok(dest)
}

/// Run the downloaded NSIS installer silently, then exit the current process.
///
/// The NSIS `/S` flag performs a silent install. The installer will close
/// the running app (via `nsProcess`), replace the files, and optionally
/// relaunch. We give it a short head start then `std::process::exit(0)`.
#[cfg(windows)]
pub fn run_installer_and_exit(path: &std::path::Path) -> AppResult<()> {
    use std::process::Command;
    tracing::info!(target: "updater", "launching silent installer: {}", path.display());
    Command::new(path)
        .arg("/S") // NSIS silent install
        .spawn()
        .map_err(|e| AppError::other(format!("启动安装程序失败: {}", e)))?;
    // Give the installer a moment to start before we exit.
    std::thread::sleep(std::time::Duration::from_millis(500));
    std::process::exit(0);
}

#[cfg(not(windows))]
pub fn run_installer_and_exit(_path: &std::path::Path) -> AppResult<()> {
    Err(AppError::other("自动更新仅支持 Windows"))
}

/// Simple semver comparison. Returns true if `remote` is strictly newer than
/// `local`. Only handles `x.y.z` — no pre-release tags.
pub fn is_newer(local: &str, remote: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.trim()
            .trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let l = parse(local);
    let r = parse(remote);
    for i in 0..3 {
        let lv = l.get(i).copied().unwrap_or(0);
        let rv = r.get(i).copied().unwrap_or(0);
        if rv > lv {
            return true;
        }
        if rv < lv {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("0.4.0", "0.5.0"));
        assert!(is_newer("0.4.0", "1.0.0"));
        assert!(is_newer("0.4.0", "0.4.1"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.5.0", "0.4.0"));
        assert!(is_newer("v0.4.0", "v0.5.0"));
    }
}
