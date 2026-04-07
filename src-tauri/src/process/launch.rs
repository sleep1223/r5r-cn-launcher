use crate::error::{AppError, AppResult};
use crate::events::{LaunchExitedEvent, EVT_LAUNCH_EXITED};
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

pub async fn launch_game(
    app: &AppHandle,
    install_dir: &Path,
    args: Vec<String>,
) -> AppResult<u32> {
    let exe = install_dir.join("r5apex.exe");
    if !exe.exists() {
        return Err(AppError::NotFound(format!(
            "未在安装目录找到 r5apex.exe: {}",
            install_dir.display()
        )));
    }

    let cmd = app
        .shell()
        .command(exe.to_string_lossy().to_string())
        .args(args)
        .current_dir(install_dir);

    let (mut rx, child) = cmd
        .spawn()
        .map_err(|e| AppError::other(format!("启动 r5apex.exe 失败: {}", e)))?;
    let pid = child.pid();

    // Drop the child so the game survives the launcher closing.
    drop(child);

    // Spawn a watcher that waits for the Terminated event and emits to JS.
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let CommandEvent::Terminated(payload) = event {
                let _ = app_clone.emit(
                    EVT_LAUNCH_EXITED,
                    LaunchExitedEvent {
                        pid,
                        code: payload.code,
                        success: payload.code == Some(0),
                    },
                );
                break;
            }
        }
    });

    Ok(pid)
}
