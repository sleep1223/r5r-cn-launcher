use crate::diagnostics::{self, DiagnosticReportResult};
use crate::error::{AppError, AppResult};
use crate::state::LauncherState;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn collect_crash_diagnostics(
    state: State<'_, LauncherState>,
    destination: String,
) -> AppResult<DiagnosticReportResult> {
    let install_dir = {
        let settings = state.settings.read();
        if settings.selected_channel.trim().is_empty() {
            return Err(AppError::settings("请先选择游戏频道"));
        }
        settings
            .install_dir_for(&settings.selected_channel)
            .ok_or_else(|| AppError::settings("请先设置游戏安装位置"))?
    };
    let destination = PathBuf::from(destination);
    tauri::async_runtime::spawn_blocking(move || diagnostics::collect(&install_dir, &destination))
        .await
        .map_err(|e| AppError::other(format!("诊断任务执行失败: {e}")))?
}

#[tauri::command]
pub fn open_diagnostic_report_folder(app: AppHandle, path: String) -> AppResult<()> {
    let path = PathBuf::from(path);
    let parent = path
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or_else(|| AppError::InvalidPath("诊断包所在目录不存在".to_string()))?;
    app.opener()
        .open_path(parent.display().to_string(), None::<&str>)
        .map_err(|e| AppError::other(format!("打开诊断包目录失败: {e}")))?;
    Ok(())
}
