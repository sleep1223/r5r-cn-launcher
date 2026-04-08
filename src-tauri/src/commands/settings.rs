use crate::config::LauncherSettings;
use crate::error::{AppError, AppResult};
use crate::state::LauncherState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn load_settings(state: State<'_, LauncherState>) -> AppResult<LauncherSettings> {
    Ok(state.settings.read().clone())
}

#[tauri::command]
pub async fn save_settings(
    state: State<'_, LauncherState>,
    settings: LauncherSettings,
) -> AppResult<()> {
    // If proxy mode changed, rebuild the HTTP client BEFORE persisting, so a
    // failed rebuild surfaces as an error and we don't accept a bad config.
    let old_mode = state.settings.read().proxy_mode.clone();
    if old_mode != settings.proxy_mode {
        let mut http = state.http.write().await;
        http.rebuild(settings.proxy_mode.clone())?;
    }
    *state.settings.write() = settings;
    state.save_settings()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidation {
    pub ok: bool,
    pub normalized: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn validate_install_path(path: String) -> AppResult<PathValidation> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if path.trim().is_empty() {
        errors.push("路径不能为空".into());
    }

    if !path.is_ascii() {
        errors.push("路径不能包含中文或非 ASCII 字符（游戏会拒绝从这种路径启动）".into());
    }

    // Only warn for the *system* Program Files — i.e. one on the C: drive.
    // A `D:\Program Files` directory is just a folder name and doesn't trigger
    // the Windows admin/UAC requirement.
    let lower = path.to_ascii_lowercase();
    let is_on_c_drive = lower.starts_with("c:\\") || lower.starts_with("c:/");
    let is_in_program_files =
        lower.contains("\\program files") || lower.contains("/program files");
    if is_on_c_drive && is_in_program_files {
        warnings
            .push("安装在 C 盘 Program Files 下需要管理员权限，建议改用其他位置".into());
    }

    let normalized = if errors.is_empty() {
        let p = std::path::PathBuf::from(&path);
        if p.exists() {
            dunce::canonicalize(&p)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.clone())
        } else {
            path.clone()
        }
    } else {
        path.clone()
    };

    Ok(PathValidation {
        ok: errors.is_empty(),
        normalized,
        errors,
        warnings,
    })
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle) -> AppResult<()> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| AppError::other(format!("无法解析日志目录: {}", e)))?;
    std::fs::create_dir_all(&dir).ok();
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| AppError::other(format!("打开日志目录失败: {}", e)))?;
    Ok(())
}
