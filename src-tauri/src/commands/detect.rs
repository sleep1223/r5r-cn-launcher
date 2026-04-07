use crate::detect::{detect_existing, DetectedInstall};
use crate::error::AppResult;
use crate::state::LauncherState;
use tauri::State;

#[tauri::command]
pub async fn detect_existing_r5r(
    state: State<'_, LauncherState>,
) -> AppResult<Vec<DetectedInstall>> {
    let extra = {
        let s = state.settings.read();
        let mut v = Vec::new();
        if !s.library_root.is_empty() {
            v.push(s.library_root.clone());
        }
        if let Some(p) = &s.last_known_official_install_path {
            v.push(p.clone());
        }
        v
    };
    let found = detect_existing(&extra).await;

    // If we discovered anything, remember the first one as the "official"
    // hint so the install picker can show it next time even when detection is
    // slow or offline.
    if let Some(first) = found.first() {
        let mut s = state.settings.write();
        s.last_known_official_install_path = Some(first.path.clone());
        drop(s);
        let _ = state.save_settings();
    }
    Ok(found)
}
