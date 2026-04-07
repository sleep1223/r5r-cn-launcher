use crate::error::AppResult;
use crate::launch_options::{
    catalog, compose_launch_args, validate_launch_args, LaunchOptionCatalog, LaunchOptionSelection,
    LaunchWarning,
};

#[tauri::command]
pub fn get_launch_option_catalog() -> AppResult<&'static LaunchOptionCatalog> {
    Ok(catalog())
}

#[tauri::command]
pub fn validate_launch_args_cmd(selection: LaunchOptionSelection) -> AppResult<Vec<LaunchWarning>> {
    Ok(validate_launch_args(&selection))
}

#[tauri::command]
pub fn compose_launch_args_cmd(selection: LaunchOptionSelection) -> AppResult<Vec<String>> {
    Ok(compose_launch_args(&selection))
}
