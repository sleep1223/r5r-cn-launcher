use crate::config::LauncherSettings;
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub const AFFECTED_GAME_VERSION: &str = "2.6.51-live";

const SCRIPT_RELATIVE_PATH: &str = "platform/scripts/vscripts/ui/menu_matchmaking_utility.nut";
const ORIGINAL_COMMAND: &str = r#"ClientCommand( "LeaveMatch" )"#;
const FIXED_COMMAND: &str = r#"ClientCommand( "disconnect" )"#;
const BACKUP_EXTENSION: &str = "nut.r5r-cn-launcher.bak";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchmakingFixState {
    NotApplicable,
    Unfixed,
    Fixed,
    FileMissing,
    UnexpectedContent,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchmakingFixStatus {
    pub state: MatchmakingFixState,
    pub game_version: Option<String>,
    pub affected_version: &'static str,
    pub file_path: Option<String>,
    pub can_fix: bool,
    pub can_restore: bool,
}

pub fn check(settings: &LauncherSettings, channel: &str) -> MatchmakingFixStatus {
    let Some(install_dir) = settings.install_dir_for(channel) else {
        return status(MatchmakingFixState::NotApplicable, None, None, false, false);
    };

    let game_version = installed_version(settings, channel, &install_dir);
    if !is_affected_version(game_version.as_deref(), channel) {
        return status(
            MatchmakingFixState::NotApplicable,
            game_version,
            Some(script_path(&install_dir)),
            false,
            false,
        );
    }

    inspect_script(&install_dir, game_version)
}

pub fn apply(settings: &LauncherSettings, channel: &str) -> AppResult<MatchmakingFixStatus> {
    let install_dir = affected_install_dir(settings, channel)?;
    let path = script_path(&install_dir);
    let original = std::fs::read_to_string(&path).map_err(|error| {
        AppError::other(format!("无法读取匹配脚本 {}: {}", path.display(), error))
    })?;
    let matches = original.matches(ORIGINAL_COMMAND).count();
    if matches != 1 {
        return Err(AppError::other(if matches == 0 {
            "未找到需要修复的 LeaveMatch 调用，文件可能已修复或已被修改".to_string()
        } else {
            format!("发现 {} 处 LeaveMatch 调用，为避免误改已停止修复", matches)
        }));
    }

    let backup = backup_path(&path);
    if !backup.exists() {
        std::fs::write(&backup, original.as_bytes()).map_err(|error| {
            AppError::other(format!("无法创建复原备份 {}: {}", backup.display(), error))
        })?;
    }

    let patched = original.replacen(ORIGINAL_COMMAND, FIXED_COMMAND, 1);
    std::fs::write(&path, patched.as_bytes()).map_err(|error| {
        AppError::other(format!(
            "无法写入匹配修复，请检查游戏目录权限 {}: {}",
            path.display(),
            error
        ))
    })?;

    tracing::info!(
        target: "matchmaking_fix",
        path = %path.display(),
        version = AFFECTED_GAME_VERSION,
        "applied LeaveMatch workaround"
    );
    Ok(inspect_script(
        &install_dir,
        Some(AFFECTED_GAME_VERSION.to_string()),
    ))
}

pub fn restore(settings: &LauncherSettings, channel: &str) -> AppResult<MatchmakingFixStatus> {
    let install_dir = affected_install_dir(settings, channel)?;
    let path = script_path(&install_dir);
    let backup = backup_path(&path);
    if !backup.exists() {
        return Err(AppError::NotFound("未找到此修复创建的复原备份".into()));
    }

    let current = std::fs::read_to_string(&path).map_err(|error| {
        AppError::other(format!("无法读取匹配脚本 {}: {}", path.display(), error))
    })?;
    let matches = current.matches(FIXED_COMMAND).count();
    if matches != 1 {
        return Err(AppError::other(if matches == 0 {
            "当前文件中未找到本修复写入的 disconnect 调用，未执行复原".to_string()
        } else {
            format!("发现 {} 处 disconnect 调用，为避免误改已停止复原", matches)
        }));
    }

    // Reverse only the line changed by this workaround. This preserves any
    // unrelated edits made after the fix was applied.
    let restored = current.replacen(FIXED_COMMAND, ORIGINAL_COMMAND, 1);
    std::fs::write(&path, restored.as_bytes()).map_err(|error| {
        AppError::other(format!(
            "无法写入复原内容，请检查游戏目录权限 {}: {}",
            path.display(),
            error
        ))
    })?;
    if let Err(error) = std::fs::remove_file(&backup) {
        tracing::warn!(
            target: "matchmaking_fix",
            path = %backup.display(),
            "复原完成，但删除备份标记失败: {}",
            error
        );
    }

    tracing::info!(
        target: "matchmaking_fix",
        path = %path.display(),
        version = AFFECTED_GAME_VERSION,
        "restored LeaveMatch command"
    );
    Ok(inspect_script(
        &install_dir,
        Some(AFFECTED_GAME_VERSION.to_string()),
    ))
}

fn affected_install_dir(settings: &LauncherSettings, channel: &str) -> AppResult<PathBuf> {
    let install_dir = settings
        .install_dir_for(channel)
        .ok_or_else(|| AppError::settings("尚未配置游戏安装目录"))?;
    let version = installed_version(settings, channel, &install_dir);
    if !is_affected_version(version.as_deref(), channel) {
        return Err(AppError::other(format!(
            "该修复仅适用于游戏版本 {}",
            AFFECTED_GAME_VERSION
        )));
    }
    Ok(install_dir)
}

fn inspect_script(install_dir: &Path, game_version: Option<String>) -> MatchmakingFixStatus {
    let path = script_path(install_dir);
    let backup = backup_path(&path);
    let file_path = Some(path.clone());
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return status(
            MatchmakingFixState::FileMissing,
            game_version,
            file_path,
            false,
            false,
        );
    };

    if contents.matches(ORIGINAL_COMMAND).count() == 1 {
        return status(
            MatchmakingFixState::Unfixed,
            game_version,
            file_path,
            true,
            false,
        );
    }
    if contents.matches(FIXED_COMMAND).count() == 1 {
        return status(
            MatchmakingFixState::Fixed,
            game_version,
            file_path,
            false,
            backup.exists(),
        );
    }
    status(
        MatchmakingFixState::UnexpectedContent,
        game_version,
        file_path,
        false,
        false,
    )
}

fn installed_version(
    settings: &LauncherSettings,
    channel: &str,
    install_dir: &Path,
) -> Option<String> {
    std::fs::read_to_string(install_dir.join("build.txt"))
        .ok()
        .and_then(|contents| parse_build_version(&contents, channel))
        .or_else(|| {
            settings
                .channels
                .get(channel)
                .map(|entry| entry.version.trim().to_string())
                .filter(|version| !version.is_empty())
        })
}

fn parse_build_version(contents: &str, channel: &str) -> Option<String> {
    let raw = contents.trim().trim_start_matches('\u{feff}');
    let version = raw
        .strip_prefix("R5R-v")
        .or_else(|| raw.strip_prefix("r5r-v"))?
        .trim();
    if version.is_empty() {
        return None;
    }
    if version.contains('-') {
        Some(version.to_ascii_lowercase())
    } else {
        Some(format!(
            "{}-{}",
            version.to_ascii_lowercase(),
            canonical_channel(channel)
        ))
    }
}

fn is_affected_version(version: Option<&str>, channel: &str) -> bool {
    canonical_channel(channel) == "live"
        && version.is_some_and(|value| value.eq_ignore_ascii_case(AFFECTED_GAME_VERSION))
}

fn canonical_channel(channel: &str) -> &'static str {
    if channel.eq_ignore_ascii_case("LIVE") || channel.eq_ignore_ascii_case("live_game") {
        "live"
    } else {
        "other"
    }
}

fn script_path(install_dir: &Path) -> PathBuf {
    install_dir.join(SCRIPT_RELATIVE_PATH)
}

fn backup_path(script: &Path) -> PathBuf {
    script.with_extension(BACKUP_EXTENSION)
}

fn status(
    state: MatchmakingFixState,
    game_version: Option<String>,
    file_path: Option<PathBuf>,
    can_fix: bool,
    can_restore: bool,
) -> MatchmakingFixStatus {
    MatchmakingFixStatus {
        state,
        game_version,
        affected_version: AFFECTED_GAME_VERSION,
        file_path: file_path.map(|path| path.display().to_string()),
        can_fix,
        can_restore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn affected_settings(root: &Path) -> LauncherSettings {
        let mut settings = LauncherSettings {
            library_root: root.display().to_string(),
            selected_channel: "LIVE".into(),
            ..LauncherSettings::default()
        };
        settings.channels.insert(
            "LIVE".into(),
            crate::config::PerChannelState {
                installed: true,
                version: AFFECTED_GAME_VERSION.into(),
                ..Default::default()
            },
        );
        settings
    }

    #[test]
    fn only_the_affected_live_version_is_applicable() {
        assert!(is_affected_version(Some("2.6.51-live"), "LIVE"));
        assert!(is_affected_version(Some("2.6.51-LIVE"), "live_game"));
        assert!(!is_affected_version(Some("2.6.50-live"), "LIVE"));
        assert!(!is_affected_version(Some("2.6.51-ptu"), "PTU"));
    }

    #[test]
    fn apply_and_restore_are_reversible() {
        let temp = tempfile::tempdir().unwrap();
        let install_dir = temp.path().join("R5R Library").join("LIVE");
        let script = script_path(&install_dir);
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(install_dir.join("build.txt"), "R5R-v2.6.51").unwrap();
        std::fs::write(&script, format!("before\r\n{}\r\nafter", ORIGINAL_COMMAND)).unwrap();
        let settings = affected_settings(temp.path());

        assert_eq!(check(&settings, "LIVE").state, MatchmakingFixState::Unfixed);
        assert_eq!(
            apply(&settings, "LIVE").unwrap().state,
            MatchmakingFixState::Fixed
        );
        assert!(std::fs::read_to_string(&script)
            .unwrap()
            .contains(FIXED_COMMAND));
        assert_eq!(
            restore(&settings, "LIVE").unwrap().state,
            MatchmakingFixState::Unfixed
        );
        assert!(std::fs::read_to_string(&script)
            .unwrap()
            .contains(ORIGINAL_COMMAND));
    }
}
