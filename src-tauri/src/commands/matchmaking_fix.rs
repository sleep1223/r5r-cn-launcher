use crate::error::AppResult;
use crate::matchmaking_fix::{self, MatchmakingFixStatus};
use crate::state::LauncherState;
use tauri::State;

#[tauri::command]
pub fn check_matchmaking_fix(
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<MatchmakingFixStatus> {
    Ok(matchmaking_fix::check(&state.settings.read(), &channel))
}

#[tauri::command]
pub fn apply_matchmaking_fix(
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<MatchmakingFixStatus> {
    matchmaking_fix::apply(&state.settings.read(), &channel)
}

#[tauri::command]
pub fn restore_matchmaking_fix(
    state: State<'_, LauncherState>,
    channel: String,
) -> AppResult<MatchmakingFixStatus> {
    matchmaking_fix::restore(&state.settings.read(), &channel)
}
