use crate::error::{AppError, AppResult};
use crate::launch_options::{compose_launch_args, LaunchOptionSelection};
use crate::process::ea_app::verify_ea_app_ready;
use crate::process::launch::launch_game;
use crate::state::LauncherState;
use std::path::PathBuf;
use tauri::{AppHandle, State};

/// Launch the game.
///
/// `install_dir_override` (optional): when set, launch from this dir instead
/// of the computed `<library_root>/R5R Library/<channel>/`. Used so a user
/// with an existing official R5R install can launch via our launcher without
/// completing the online install flow.
#[tauri::command]
pub async fn launch_game_cmd(
    app: AppHandle,
    state: State<'_, LauncherState>,
    channel: String,
    selection: LaunchOptionSelection,
    install_dir_override: Option<String>,
) -> AppResult<u32> {
    let (install_dir, launch_via_ea_app): (PathBuf, bool) = {
        let s = state.settings.read();
        let dir = if let Some(p) = install_dir_override {
            PathBuf::from(p)
        } else {
            s.install_dir_for(&channel)
                .ok_or_else(|| AppError::settings("尚未配置安装位置"))?
        };
        (dir, s.launch_via_ea_app)
    };

    if launch_via_ea_app {
        verify_ea_app_ready().await?;
    }

    let args = compose_launch_args(&selection);
    launch_game(&app, &install_dir, args).await
}
