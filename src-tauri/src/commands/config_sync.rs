use crate::config_sync::{
    self, ApplyConfigRequest, ConfigApplyPreview, ConfigApplyResult, ConfigBackupRecord,
    ConfigComparison, ConfigGame, GeneratedConfigContent,
};
use crate::error::AppResult;
use crate::state::LauncherState;
use tauri::State;

#[tauri::command]
pub fn scan_game_configs() -> AppResult<ConfigComparison> {
    config_sync::scan_game_configs()
}

#[tauri::command]
pub fn generate_game_config_content(
    source_game: ConfigGame,
    selected_keys: Vec<String>,
) -> AppResult<GeneratedConfigContent> {
    config_sync::generate_config_content(source_game, selected_keys)
}

#[tauri::command]
pub fn preview_game_config_apply(
    content: String,
    selected_keys: Vec<String>,
    target_games: Vec<ConfigGame>,
) -> AppResult<ConfigApplyPreview> {
    config_sync::preview_config_apply(&content, selected_keys, target_games)
}

#[tauri::command]
pub fn apply_game_config(
    state: State<'_, LauncherState>,
    request: ApplyConfigRequest,
) -> AppResult<ConfigApplyResult> {
    let config_dir = state.config_dir.read().clone();
    config_sync::apply_config(&config_dir, request)
}

#[tauri::command]
pub fn create_game_config_backup(
    state: State<'_, LauncherState>,
    game: ConfigGame,
    label: String,
) -> AppResult<ConfigBackupRecord> {
    let config_dir = state.config_dir.read().clone();
    config_sync::create_config_backup(&config_dir, game, label)
}

#[tauri::command]
pub fn list_game_config_backups(
    state: State<'_, LauncherState>,
    game: Option<ConfigGame>,
) -> AppResult<Vec<ConfigBackupRecord>> {
    let config_dir = state.config_dir.read().clone();
    config_sync::list_config_backups(&config_dir, game)
}

#[tauri::command]
pub fn delete_game_config_backup(
    state: State<'_, LauncherState>,
    backup_id: String,
) -> AppResult<()> {
    let config_dir = state.config_dir.read().clone();
    config_sync::delete_config_backup(&config_dir, &backup_id)
}

#[tauri::command]
pub fn restore_game_config_backup(
    state: State<'_, LauncherState>,
    backup_id: String,
) -> AppResult<ConfigBackupRecord> {
    let config_dir = state.config_dir.read().clone();
    config_sync::restore_config_backup(&config_dir, &backup_id)
}

#[tauri::command]
pub fn restore_latest_game_config_backup(
    state: State<'_, LauncherState>,
    game: ConfigGame,
) -> AppResult<ConfigBackupRecord> {
    let config_dir = state.config_dir.read().clone();
    config_sync::restore_latest_config_backup(&config_dir, game)
}
