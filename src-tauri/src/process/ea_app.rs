//! Start EA App and make sure a user profile is present before launching.
//!
//! R5Reloaded does not strictly require EA App, but launching while EA is open
//! at the sign-in screen is a common footgun. The official launcher starts EA
//! from `DesktopAppPath`; we do the same, then add a conservative login check:
//! EA App creates `%LOCALAPPDATA%\Electronic Arts\EA Desktop\user_*.ini` for
//! the current EA user. We only inspect the file name/metadata, never tokens.

use crate::error::{AppError, AppResult};

#[cfg(windows)]
const EA_PROCESS_NAME: &str = "eadesktop.exe";
#[cfg(windows)]
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
#[cfg(windows)]
const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(windows)]
const LOGIN_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Ensure EA App is running and appears to have a logged-in user profile.
#[cfg(windows)]
pub async fn ensure_ea_app_running() -> AppResult<()> {
    if !is_ea_app_running() {
        spawn_ea_app()?;

        let deadline = std::time::Instant::now() + WAIT_TIMEOUT;
        while std::time::Instant::now() < deadline {
            tokio::time::sleep(POLL_INTERVAL).await;
            if is_ea_app_running() {
                break;
            }
        }

        if !is_ea_app_running() {
            return Err(AppError::other(
                "EA App 已尝试启动，但进程没有出现。请手动打开 EA App 后重试。",
            ));
        }
    } else if !is_ea_app_logged_in() {
        // Bring the existing EA App window forward so the user sees the login
        // prompt instead of wondering why the launcher refuses to continue.
        let _ = spawn_ea_app();
    }

    if is_ea_app_logged_in() {
        return Ok(());
    }

    let deadline = std::time::Instant::now() + LOGIN_WAIT_TIMEOUT;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(POLL_INTERVAL).await;
        if is_ea_app_logged_in() {
            return Ok(());
        }
    }

    Err(AppError::other(
        "EA App 已打开，但尚未检测到登录用户。请先在 EA App 完成登录后再启动游戏。",
    ))
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
fn is_ea_app_logged_in() -> bool {
    let Some(dir) = ea_desktop_data_dir() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };

    entries.filter_map(Result::ok).any(|entry| {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("user_") || !name.ends_with(".ini") {
            return false;
        }
        entry
            .metadata()
            .map(|m| m.is_file() && m.len() > 0)
            .unwrap_or(false)
    })
}

#[cfg(windows)]
fn ea_desktop_data_dir() -> Option<std::path::PathBuf> {
    let mut dir = std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    dir.push("Electronic Arts");
    dir.push("EA Desktop");
    Some(dir)
}

#[cfg(windows)]
fn spawn_ea_app() -> AppResult<()> {
    use std::path::PathBuf;
    use std::process::Command;

    // 1. Registry: match the official launcher path first, with a native
    // fallback for machines that write the 64-bit key.
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
    for subkey in [
        r"SOFTWARE\WOW6432Node\Electronic Arts\EA Desktop",
        r"SOFTWARE\Electronic Arts\EA Desktop",
    ] {
        let Ok(key) = hklm.open_subkey(subkey) else {
            continue;
        };
        for value in ["DesktopAppPath", "InstallLocation"] {
            let Ok(raw) = key.get_value::<String, _>(value) else {
                continue;
            };
            let path = normalize_ea_app_path(raw);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(windows)]
fn normalize_ea_app_path(raw: String) -> std::path::PathBuf {
    let trimmed = raw.trim().trim_matches('"');
    let mut path = std::path::PathBuf::from(trimmed);
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("EADesktop.exe"))
        .unwrap_or(false)
    {
        return path;
    }
    path.push("EADesktop.exe");
    path
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
