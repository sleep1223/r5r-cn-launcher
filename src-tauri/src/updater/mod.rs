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
pub async fn download_installer(app: &AppHandle, client: &Client, url: &str) -> AppResult<PathBuf> {
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

/// Spawn a fully windowless VBScript helper that waits for us to exit, runs
/// the NSIS installer silently, then relaunches the updated exe. Then exit
/// immediately.
///
/// Why VBScript instead of cmd/PowerShell: `wscript.exe` is a GUI-subsystem
/// program with no console, so there is never a visible black window. cmd —
/// even with `CREATE_NO_WINDOW` — flashes a console on some Windows 10/11
/// machines (conhost / Windows Terminal takeover quirks), and PowerShell has
/// its own ~100 ms startup flash. VBScript via `WScript.Shell.Run(cmd, 0, True)`
/// also *blocks* on the installer, which is far more reliable than guessing
/// at sleep durations.
///
/// The script is written as UTF-16 LE with BOM because wscript.exe falls back
/// to the system ANSI codepage when no BOM is present. On a Chinese Windows
/// that's GBK, which mangles any Unicode char in the exe path
/// (e.g. `C:\Users\张三\...`) and breaks the relaunch.
#[cfg(windows)]
pub fn run_installer_and_exit(path: &std::path::Path) -> AppResult<()> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let current_exe = std::env::current_exe()
        .map_err(|e| AppError::other(format!("无法获取当前 exe 路径: {}", e)))?;

    // Embedded `"` inside a VBS string literal must be doubled. Paths with
    // literal quotes are pathological but the replacement is cheap.
    let escape_vbs = |s: String| s.replace('"', "\"\"");
    let installer_vbs = escape_vbs(path.display().to_string());
    let exe_vbs = escape_vbs(current_exe.display().to_string());

    let script = format!(
        "Dim sh : Set sh = CreateObject(\"WScript.Shell\")\r\n\
         WScript.Sleep 1000\r\n\
         sh.Run \"\"\"{installer}\"\" /S\", 0, True\r\n\
         WScript.Sleep 500\r\n\
         sh.Run \"\"\"{exe}\"\"\", 1, False\r\n",
        installer = installer_vbs,
        exe = exe_vbs,
    );

    // UTF-16 LE + BOM so wscript.exe uses Unicode regardless of the system
    // codepage.
    let mut bytes = vec![0xFF, 0xFE];
    for unit in script.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    let helper_path = std::env::temp_dir().join("r5r-cn-launcher-update.vbs");
    std::fs::write(&helper_path, &bytes)
        .map_err(|e| AppError::other(format!("写入更新助手脚本失败: {}", e)))?;

    tracing::info!(
        target: "updater",
        "spawning update helper: {} (installer={}, relaunch={})",
        helper_path.display(),
        path.display(),
        current_exe.display()
    );

    Command::new("wscript.exe")
        .arg(&helper_path)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::other(format!("启动更新助手失败: {}", e)))?;

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
