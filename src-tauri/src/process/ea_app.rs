//! Start EA App and wait for its process to come up before launching the game.
//!
//! R5Reloaded does not strictly require EA App, but many users want it running
//! so the in-game friends overlay / Origin telemetry works. The flow is:
//!   1. If `EADesktop.exe` is already running → done.
//!   2. Otherwise spawn EA App (registry path → common install paths → URL
//!      scheme `eadesktop://`). If none of those launch anything → error.
//!   3. Poll up to 5s for the process to appear. Timeout is non-fatal — we
//!      proceed to the game launch anyway.

use crate::error::AppResult;

#[cfg(windows)]
const EA_PROCESS_NAME: &str = "eadesktop.exe";
#[cfg(windows)]
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
#[cfg(windows)]
const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Best-effort: ensure EA App is running before launching the game.
///
/// Returns `Err` only when EA App cannot be located on this machine — the
/// caller should surface that to the user. A 5s wait timeout after a
/// successful spawn is treated as non-fatal (we return `Ok`).
#[cfg(windows)]
pub async fn ensure_ea_app_running() -> AppResult<()> {
    if is_ea_app_running() {
        return Ok(());
    }

    spawn_ea_app()?;

    let deadline = std::time::Instant::now() + WAIT_TIMEOUT;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL_INTERVAL).await;
        if is_ea_app_running() {
            return Ok(());
        }
    }
    // Timeout — non-fatal, just proceed.
    tracing::warn!("EA App spawn succeeded but process didn't appear within {:?}", WAIT_TIMEOUT);
    Ok(())
}

/// Stub for non-Windows dev (mac). EA App is Windows-only; on other platforms
/// we silently skip the pre-launch step so `pnpm tauri dev` still works.
#[cfg(not(windows))]
pub async fn ensure_ea_app_running() -> AppResult<()> {
    Ok(())
}

#[cfg(windows)]
fn is_ea_app_running() -> bool {
    use sysinfo::{ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        p.name()
            .to_string_lossy()
            .eq_ignore_ascii_case(EA_PROCESS_NAME)
    })
}

#[cfg(windows)]
fn spawn_ea_app() -> AppResult<()> {
    use crate::error::AppError;
    use std::path::PathBuf;
    use std::process::Command;

    // 1. Registry: HKLM\SOFTWARE\Electronic Arts\EA Desktop -> DesktopAppPath
    if let Some(path) = registry_path() {
        if path.exists() {
            return spawn_detached(&path);
        }
    }

    // 2. Common install locations.
    let candidates = [
        r"C:\Program Files\Electronic Arts\EA Desktop\EA Desktop\EADesktop.exe",
        r"C:\Program Files (x86)\Electronic Arts\EA Desktop\EA Desktop\EADesktop.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return spawn_detached(&p);
        }
    }

    // 3. URL scheme — works if EA App registered its handler (`eadesktop://`).
    // `cmd /c start "" eadesktop://` returns immediately whether or not a
    // handler is registered, so we treat success of this command as a soft
    // signal: we'll still rely on the post-spawn poll to confirm.
    let r = Command::new("cmd")
        .args(["/c", "start", "", "eadesktop://"])
        .spawn();
    if r.is_ok() {
        return Ok(());
    }

    Err(AppError::other(
        "未能启动 EA App，请确认已安装 EA App 后重试，或在设置中关闭“启动前先打开 EA App”。",
    ))
}

#[cfg(windows)]
fn registry_path() -> Option<std::path::PathBuf> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm
        .open_subkey(r"SOFTWARE\Electronic Arts\EA Desktop")
        .ok()?;
    let dir: String = key.get_value("DesktopAppPath").ok()?;
    let mut p = std::path::PathBuf::from(dir);
    p.push("EADesktop.exe");
    Some(p)
}

#[cfg(windows)]
fn spawn_detached(exe: &std::path::Path) -> AppResult<()> {
    use crate::error::AppError;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    // DETACHED_PROCESS | CREATE_NO_WINDOW so EA App survives the launcher
    // closing and doesn't briefly flash a console.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new(exe)
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| AppError::other(format!("启动 EA App 失败: {}", e)))?;
    Ok(())
}
