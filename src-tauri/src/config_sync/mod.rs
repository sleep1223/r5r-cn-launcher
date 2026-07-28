use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, System};
use uuid::Uuid;

const SETTINGS_RELATIVE_PATH: &str = "local/settings.cfg";
const PROFILE_RELATIVE_PATH: &str = "profile/profile.cfg";
const MAX_CONTENT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigGame {
    Apex,
    R5,
}

impl ConfigGame {
    fn folder_name(self) -> &'static str {
        match self {
            Self::Apex => "Apex",
            Self::R5 => "Apex_fnf",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Apex => "apex",
            Self::R5 => "r5",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSection {
    MouseKeyboard,
    Controller,
    Fov,
}

#[derive(Debug, Clone)]
struct FieldDefinition {
    key: String,
    label: String,
    section: ConfigSection,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigEntryState {
    pub key: String,
    pub label: String,
    pub section: ConfigSection,
    pub present: bool,
    pub value: Option<String>,
    pub values: Vec<String>,
    pub locations: Vec<String>,
    pub conflict: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFileFingerprint {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameConfigSnapshot {
    pub game: ConfigGame,
    pub detected: bool,
    pub root_path: String,
    pub files: Vec<ConfigFileFingerprint>,
    pub entries: Vec<ConfigEntryState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigComparison {
    pub apex: GameConfigSnapshot,
    pub r5: GameConfigSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedConfigContent {
    pub source_game: ConfigGame,
    pub content: String,
    pub keys: Vec<String>,
    pub has_mouse: bool,
    pub has_controller: bool,
    pub has_fov: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewStatus {
    Replace,
    Unchanged,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPreviewItem {
    pub key: String,
    pub label: String,
    pub section: ConfigSection,
    pub desired: String,
    pub current_values: Vec<String>,
    pub status: PreviewStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetApplyPreview {
    pub game: ConfigGame,
    pub files: Vec<ConfigFileFingerprint>,
    pub replace_count: usize,
    pub unchanged_count: usize,
    pub missing_count: usize,
    pub conflict_count: usize,
    pub items: Vec<ApplyPreviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigApplyPreview {
    pub content_sha256: String,
    pub selected_keys: Vec<String>,
    pub targets: Vec<TargetApplyPreview>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyConfigRequest {
    pub content: String,
    pub selected_keys: Vec<String>,
    pub target_games: Vec<ConfigGame>,
    pub expected_preview: ConfigApplyPreview,
    pub source_label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApplyTargetResult {
    pub game: ConfigGame,
    pub replaced: usize,
    pub unchanged: usize,
    pub missing: usize,
    pub backup_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigApplyResult {
    pub targets: Vec<ApplyTargetResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFileRecord {
    pub relative_path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBackupRecord {
    pub id: String,
    pub game: ConfigGame,
    pub created_at_ms: u64,
    pub label: String,
    pub operation_source: String,
    pub files: Vec<BackupFileRecord>,
    pub total_size: u64,
}

#[derive(Debug, Clone)]
struct ParsedLine<'a> {
    key: &'a str,
    value: &'a str,
    value_start: usize,
    value_end: usize,
}

#[derive(Debug, Clone)]
struct ContentEntry {
    key: String,
    value: String,
    definition: FieldDefinition,
}

#[derive(Debug, Clone)]
struct PreparedFile {
    path: PathBuf,
    new_bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
struct SwapState {
    target: PathBuf,
    temp: Option<PathBuf>,
    rollback: Option<PathBuf>,
    target_existed: bool,
    touched: bool,
}

pub fn scan_game_configs() -> AppResult<ConfigComparison> {
    Ok(ConfigComparison {
        apex: scan_game(ConfigGame::Apex)?,
        r5: scan_game(ConfigGame::R5)?,
    })
}

pub fn generate_config_content(
    source_game: ConfigGame,
    selected_keys: Vec<String>,
) -> AppResult<GeneratedConfigContent> {
    let snapshot = scan_game(source_game)?;
    generate_config_content_from_snapshot(source_game, selected_keys, snapshot)
}

fn generate_config_content_from_snapshot(
    source_game: ConfigGame,
    selected_keys: Vec<String>,
    snapshot: GameConfigSnapshot,
) -> AppResult<GeneratedConfigContent> {
    if !snapshot.detected {
        return Err(AppError::NotFound(format!(
            "未检测到 {} 配置",
            source_game.folder_name()
        )));
    }

    let selected: HashSet<&str> = selected_keys.iter().map(String::as_str).collect();
    let mut lines = Vec::new();
    let mut keys = Vec::new();
    let mut has_mouse = false;
    let mut has_controller = false;
    let mut has_fov = false;

    for entry in &snapshot.entries {
        if !selected.contains(entry.key.as_str()) {
            continue;
        }
        if entry.conflict {
            return Err(AppError::other(format!(
                "配置项 {} 在来源文件中存在冲突值，无法生成档案",
                entry.key
            )));
        }
        let Some(value) = &entry.value else {
            continue;
        };
        lines.push(format!("{} \"{}\"", entry.key, value));
        keys.push(entry.key.clone());
        match entry.section {
            ConfigSection::MouseKeyboard => has_mouse = true,
            ConfigSection::Controller => has_controller = true,
            ConfigSection::Fov => has_fov = true,
        }
    }

    if lines.is_empty() {
        return Err(AppError::other("没有可生成的配置项"));
    }

    Ok(GeneratedConfigContent {
        source_game,
        content: lines.join("\n"),
        keys,
        has_mouse,
        has_controller,
        has_fov,
    })
}

pub fn preview_config_apply(
    content: &str,
    selected_keys: Vec<String>,
    target_games: Vec<ConfigGame>,
) -> AppResult<ConfigApplyPreview> {
    if target_games.is_empty() {
        return Err(AppError::other("请至少选择一个目标游戏"));
    }
    let target_games = unique_games(target_games);
    let entries = selected_content_entries(content, &selected_keys)?;
    let canonical = canonical_content(&entries);
    let mut targets = Vec::new();
    for game in target_games {
        targets.push(analyze_target(game, &entries)?);
    }
    Ok(ConfigApplyPreview {
        content_sha256: hash_bytes(canonical.as_bytes()),
        selected_keys: entries.iter().map(|entry| entry.key.clone()).collect(),
        targets,
    })
}

pub fn apply_config(
    config_dir: &Path,
    request: ApplyConfigRequest,
) -> AppResult<ConfigApplyResult> {
    ensure_game_not_running()?;
    let target_games = unique_games(request.target_games);
    let fresh_preview = preview_config_apply(
        &request.content,
        request.selected_keys.clone(),
        target_games.clone(),
    )?;
    if fresh_preview.content_sha256 != request.expected_preview.content_sha256
        || fresh_preview.selected_keys != request.expected_preview.selected_keys
        || fresh_preview.targets.len() != request.expected_preview.targets.len()
    {
        return Err(AppError::other("配置内容或目标已变化，请重新预览"));
    }
    for target in &fresh_preview.targets {
        let expected = request
            .expected_preview
            .targets
            .iter()
            .find(|item| item.game == target.game)
            .ok_or_else(|| AppError::other("预览中缺少目标游戏"))?;
        if expected.files != target.files {
            return Err(AppError::other(format!(
                "{} 配置文件已变化，请重新预览",
                target.game.folder_name()
            )));
        }
    }

    let entries = selected_content_entries(&request.content, &request.selected_keys)?;
    let mut prepared = Vec::new();
    let mut backups = HashMap::new();
    for target in &fresh_preview.targets {
        if target.replace_count > 0 {
            ensure_game_files_writable(target.game)?;
        }
    }
    for target in &fresh_preview.targets {
        if target.replace_count == 0 {
            continue;
        }
        let backup = create_backup_internal(
            config_dir,
            target.game,
            format!("应用前自动备份：{}", request.source_label.trim()),
            format!("apply:{}", request.source_label.trim()),
            false,
        )?;
        backups.insert(target.game, backup.id.clone());
        prepared.extend(build_rewritten_files(target.game, &entries)?);
    }

    if !prepared.is_empty() {
        replace_files_transactionally(prepared)?;
    }

    Ok(ConfigApplyResult {
        targets: fresh_preview
            .targets
            .into_iter()
            .map(|target| ApplyTargetResult {
                game: target.game,
                replaced: target.replace_count,
                unchanged: target.unchanged_count,
                missing: target.missing_count,
                backup_id: backups.get(&target.game).cloned(),
            })
            .collect(),
    })
}

pub fn create_config_backup(
    config_dir: &Path,
    game: ConfigGame,
    label: String,
) -> AppResult<ConfigBackupRecord> {
    let label = if label.trim().is_empty() {
        "手动备份".to_string()
    } else {
        label.trim().to_string()
    };
    create_backup_internal(config_dir, game, label, "manual".to_string(), false)
}

pub fn list_config_backups(
    config_dir: &Path,
    game: Option<ConfigGame>,
) -> AppResult<Vec<ConfigBackupRecord>> {
    let mut records = Vec::new();
    let games: Vec<ConfigGame> = match game {
        Some(game) => vec![game],
        None => vec![ConfigGame::Apex, ConfigGame::R5],
    };
    for game in games {
        let root = backup_game_root(config_dir, game);
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let manifest = entry.path().join("manifest.json");
            let Ok(bytes) = fs::read(&manifest) else {
                continue;
            };
            let Ok(record) = serde_json::from_slice::<ConfigBackupRecord>(&bytes) else {
                continue;
            };
            records.push(record);
        }
    }
    records.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
    Ok(records)
}

pub fn delete_config_backup(config_dir: &Path, backup_id: &str) -> AppResult<()> {
    let (_, path, _) = find_backup(config_dir, backup_id)?;
    let root = config_dir.join("game-config-sync").join("backups");
    let canonical_root = fs::canonicalize(&root)?;
    let canonical_path = fs::canonicalize(&path)?;
    if canonical_path == canonical_root || !canonical_path.starts_with(&canonical_root) {
        return Err(AppError::InvalidPath("备份目录越界".into()));
    }
    fs::remove_dir_all(canonical_path)?;
    Ok(())
}

pub fn restore_config_backup(config_dir: &Path, backup_id: &str) -> AppResult<ConfigBackupRecord> {
    ensure_game_not_running()?;
    let (record, backup_dir, _) = find_backup(config_dir, backup_id)?;
    ensure_game_files_writable(record.game)?;
    let _undo_backup = create_backup_internal(
        config_dir,
        record.game,
        format!("恢复前自动备份：{}", record.label),
        format!("restore:{}", record.id),
        true,
    )?;
    restore_record(&record, &backup_dir)?;
    Ok(record)
}

pub fn restore_latest_config_backup(
    config_dir: &Path,
    game: ConfigGame,
) -> AppResult<ConfigBackupRecord> {
    let latest = list_config_backups(config_dir, Some(game))?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound("没有可恢复的备份".into()))?;
    restore_config_backup(config_dir, &latest.id)
}

fn field_definitions() -> Vec<FieldDefinition> {
    let mut fields = vec![
        field(
            "mouse_sensitivity",
            "鼠标灵敏度",
            ConfigSection::MouseKeyboard,
        ),
        field(
            "mouse_use_per_scope_sensitivity_scalars",
            "启用每倍镜 ADS 鼠标灵敏度",
            ConfigSection::MouseKeyboard,
        ),
    ];
    for (index, optic) in OPTIC_NAMES.iter().enumerate() {
        let label = match index {
            0 => "ADS 鼠标灵敏度加成（默认 / 1x 光学缩放 / 瞄具）".to_string(),
            7 => "未使用瞄具的 ADS 鼠标灵敏度加成（保留项）".to_string(),
            _ => format!("{optic} ADS 鼠标灵敏度加成"),
        };
        fields.push(field(
            &format!("mouse_zoomed_sensitivity_scalar_{index}"),
            &label,
            ConfigSection::MouseKeyboard,
        ));
    }
    fields.push(field("cl_fovScale", "视野 (FOV)", ConfigSection::Fov));
    fields.push(field(
        "gamepad_aim_speed",
        "视线灵敏度",
        ConfigSection::Controller,
    ));
    for (index, optic) in OPTIC_NAMES.iter().enumerate() {
        let label = match index {
            0 => "ADS 视线灵敏度（默认 / 1x 光学缩放 / 瞄具）".to_string(),
            7 => "未使用瞄具的 ADS 视线灵敏度（保留项）".to_string(),
            _ => format!("{optic} ADS 视线灵敏度"),
        };
        fields.push(field(
            &format!("gamepad_aim_speed_ads_{index}"),
            &label,
            ConfigSection::Controller,
        ));
    }
    for (index, optic) in OPTIC_NAMES.iter().enumerate() {
        let label = if index == 7 {
            "未使用瞄具的 ADS 灵敏度加成（保留项）".to_string()
        } else {
            format!("{optic} ADS 灵敏度加成")
        };
        fields.push(field(
            &format!("gamepad_ads_advanced_sensitivity_scalar_{index}"),
            &label,
            ConfigSection::Controller,
        ));
    }
    fields.extend([
        field(
            "gamepad_use_per_scope_sensitivity_scalars",
            "启用每倍镜 ADS 灵敏度倍率",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_use_per_scope_ads_settings",
            "启用每倍镜 ADS 视线灵敏度",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_enabled",
            "自定义视角控制（ALC）",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_ads_pitch",
            "ALC ADS 上下移动速度",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_ads_yaw",
            "ALC ADS 左右移动速度",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_ads_turn_pitch",
            "ALC ADS 转向额外上下移动",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_ads_turn_yaw",
            "ALC ADS 转向额外左右移动",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_ads_turn_time",
            "ALC ADS 转向启动时间",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_ads_turn_delay",
            "ALC ADS 转向启动延迟",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_hip_pitch",
            "ALC 上下移动速度",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_hip_yaw",
            "ALC 左右移动速度",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_hip_turn_pitch",
            "ALC 转向额外上下移动",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_hip_turn_yaw",
            "ALC 转向额外左右移动",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_hip_turn_time",
            "ALC 转向启动时间",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_hip_turn_delay",
            "ALC 转向启动延迟",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_curve",
            "ALC 响应曲线",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_deadzone_in",
            "ALC 死区",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_custom_deadzone_out",
            "ALC 外部阈值",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_deadzone_index_look",
            "视野死角",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_deadzone_index_move",
            "移动死角",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_look_curve",
            "基础响应曲线",
            ConfigSection::Controller,
        ),
        field(
            "gamepad_trigger_threshold",
            "扳机键盲区",
            ConfigSection::Controller,
        ),
    ]);
    fields
}

const OPTIC_NAMES: [&str; 8] = [
    "1x 光学缩放 / 瞄具",
    "2x 光学缩放",
    "3x 光学缩放",
    "4x 光学缩放",
    "6x 光学缩放",
    "8x 光学缩放",
    "10x 光学缩放",
    "未使用瞄具",
];

fn field(key: &str, label: &str, section: ConfigSection) -> FieldDefinition {
    FieldDefinition {
        key: key.to_string(),
        label: label.to_string(),
        section,
    }
}

fn definition_map() -> HashMap<String, FieldDefinition> {
    field_definitions()
        .into_iter()
        .map(|definition| (definition.key.clone(), definition))
        .collect()
}

fn scan_game(game: ConfigGame) -> AppResult<GameConfigSnapshot> {
    let root = game_root(game)?;
    scan_game_at(game, root)
}

fn scan_game_at(game: ConfigGame, root: PathBuf) -> AppResult<GameConfigSnapshot> {
    let definitions = field_definitions();
    let allowed: HashSet<&str> = definitions.iter().map(|item| item.key.as_str()).collect();
    let mut occurrences: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut files = Vec::new();

    for relative in [SETTINGS_RELATIVE_PATH, PROFILE_RELATIVE_PATH] {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path)?;
        let content = String::from_utf8(bytes.clone())
            .map_err(|_| AppError::other(format!("配置文件不是有效 UTF-8：{}", path.display())))?;
        files.push(ConfigFileFingerprint {
            path: path.to_string_lossy().to_string(),
            sha256: hash_bytes(&bytes),
            size: bytes.len() as u64,
        });
        for line in content.split_inclusive('\n') {
            let Ok(Some(parsed)) = parse_line(line, false) else {
                continue;
            };
            if allowed.contains(parsed.key) {
                occurrences
                    .entry(parsed.key.to_string())
                    .or_default()
                    .push((relative.to_string(), parsed.value.to_string()));
            }
        }
    }

    let entries = definitions
        .into_iter()
        .map(|definition| {
            let found = occurrences.remove(&definition.key).unwrap_or_default();
            let mut values = Vec::new();
            let mut locations = Vec::new();
            for (location, value) in found {
                if !locations.contains(&location) {
                    locations.push(location);
                }
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            let conflict = values.len() > 1;
            ConfigEntryState {
                key: definition.key,
                label: definition.label,
                section: definition.section,
                present: !values.is_empty(),
                value: if values.len() == 1 {
                    Some(values[0].clone())
                } else {
                    None
                },
                values,
                locations,
                conflict,
            }
        })
        .collect();

    Ok(GameConfigSnapshot {
        game,
        detected: !files.is_empty(),
        root_path: root.to_string_lossy().to_string(),
        files,
        entries,
    })
}

fn selected_content_entries(
    content: &str,
    selected_keys: &[String],
) -> AppResult<Vec<ContentEntry>> {
    let entries = parse_content(content)?;
    let selected: HashSet<&str> = selected_keys.iter().map(String::as_str).collect();
    if selected.is_empty() {
        return Err(AppError::other("请至少选择一个配置项"));
    }
    let filtered: Vec<ContentEntry> = entries
        .into_iter()
        .filter(|entry| selected.contains(entry.key.as_str()))
        .collect();
    if filtered.is_empty() {
        return Err(AppError::other("所选配置项不在档案内容中"));
    }
    if filtered.len() != selected.len() {
        return Err(AppError::other("部分所选配置项不在档案内容中"));
    }
    Ok(filtered)
}

fn parse_content(content: &str) -> AppResult<Vec<ContentEntry>> {
    if content.as_bytes().len() > MAX_CONTENT_BYTES {
        return Err(AppError::other("配置内容超过 16 KiB"));
    }
    let definitions = definition_map();
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for (index, line) in content.split_inclusive('\n').enumerate() {
        let parsed = parse_line(line, true)
            .map_err(|message| AppError::other(format!("第 {} 行：{}", index + 1, message)))?;
        let Some(parsed) = parsed else {
            continue;
        };
        let definition = definitions.get(parsed.key).ok_or_else(|| {
            AppError::other(format!(
                "第 {} 行包含不允许的配置项 {}",
                index + 1,
                parsed.key
            ))
        })?;
        if !is_finite_number(parsed.value) {
            return Err(AppError::other(format!(
                "第 {} 行的值不是有限数字",
                index + 1
            )));
        }
        if !seen.insert(parsed.key.to_string()) {
            return Err(AppError::other(format!("配置项 {} 重复", parsed.key)));
        }
        entries.push(ContentEntry {
            key: parsed.key.to_string(),
            value: parsed.value.to_string(),
            definition: definition.clone(),
        });
    }
    if entries.is_empty() {
        return Err(AppError::other("配置内容为空"));
    }
    let order: HashMap<String, usize> = field_definitions()
        .into_iter()
        .enumerate()
        .map(|(index, item)| (item.key, index))
        .collect();
    entries.sort_by_key(|entry| order.get(&entry.key).copied().unwrap_or(usize::MAX));
    Ok(entries)
}

fn parse_line(line: &str, strict: bool) -> Result<Option<ParsedLine<'_>>, String> {
    let mut offset = 0;
    let body = if let Some(rest) = line.strip_prefix('\u{feff}') {
        offset = '\u{feff}'.len_utf8();
        rest
    } else {
        line
    };
    let leading = body.len() - body.trim_start_matches([' ', '\t']).len();
    offset += leading;
    let trimmed = &body[leading..];
    if trimmed.trim_matches([' ', '\t', '\r', '\n']).is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with("//") || trimmed.starts_with('#') {
        return if strict {
            Err("不允许注释行".into())
        } else {
            Ok(None)
        };
    }
    let key_end = trimmed
        .find(|ch: char| ch.is_ascii_whitespace())
        .ok_or_else(|| "缺少配置值".to_string())?;
    let key = &trimmed[..key_end];
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
    {
        return Err("配置项名称无效".into());
    }
    let after_key = &trimmed[key_end..];
    let whitespace = after_key.len() - after_key.trim_start_matches([' ', '\t']).len();
    let quoted = &after_key[whitespace..];
    if !quoted.starts_with('"') {
        return Err("配置值必须使用双引号".into());
    }
    let value_tail = &quoted[1..];
    let closing = value_tail
        .find('"')
        .ok_or_else(|| "配置值缺少结束引号".to_string())?;
    let value = &value_tail[..closing];
    let suffix = &value_tail[closing + 1..];
    let trimmed_suffix = suffix.trim_matches([' ', '\t', '\r', '\n']);
    if strict && !trimmed_suffix.is_empty() {
        return Err("配置行包含额外内容".into());
    }
    if !strict && !trimmed_suffix.is_empty() && !trimmed_suffix.starts_with("//") {
        return Err("配置行包含额外命令".into());
    }
    let value_start = offset + key_end + whitespace + 1;
    Ok(Some(ParsedLine {
        key,
        value,
        value_start,
        value_end: value_start + value.len(),
    }))
}

fn is_finite_number(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .parse::<f64>()
            .map(|number| number.is_finite())
            .unwrap_or(false)
}

fn analyze_target(game: ConfigGame, entries: &[ContentEntry]) -> AppResult<TargetApplyPreview> {
    let snapshot = scan_game(game)?;
    analyze_target_snapshot(game, entries, snapshot)
}

fn analyze_target_snapshot(
    game: ConfigGame,
    entries: &[ContentEntry],
    snapshot: GameConfigSnapshot,
) -> AppResult<TargetApplyPreview> {
    if !snapshot.detected {
        return Err(AppError::NotFound(format!(
            "未检测到 {} 配置",
            game.folder_name()
        )));
    }
    let states: HashMap<&str, &ConfigEntryState> = snapshot
        .entries
        .iter()
        .map(|state| (state.key.as_str(), state))
        .collect();
    let mut items = Vec::new();
    let mut replace_count = 0;
    let mut unchanged_count = 0;
    let mut missing_count = 0;
    let mut conflict_count = 0;
    for entry in entries {
        let state = states
            .get(entry.key.as_str())
            .copied()
            .ok_or_else(|| AppError::other(format!("未知配置项 {}", entry.key)))?;
        let status = if !state.present {
            missing_count += 1;
            PreviewStatus::Missing
        } else if state.values.iter().all(|value| value == &entry.value) {
            unchanged_count += 1;
            PreviewStatus::Unchanged
        } else {
            replace_count += 1;
            if state.conflict {
                conflict_count += 1;
            }
            PreviewStatus::Replace
        };
        items.push(ApplyPreviewItem {
            key: entry.key.clone(),
            label: entry.definition.label.clone(),
            section: entry.definition.section,
            desired: entry.value.clone(),
            current_values: state.values.clone(),
            status,
        });
    }
    Ok(TargetApplyPreview {
        game,
        files: snapshot.files,
        replace_count,
        unchanged_count,
        missing_count,
        conflict_count,
        items,
    })
}

fn build_rewritten_files(
    game: ConfigGame,
    entries: &[ContentEntry],
) -> AppResult<Vec<PreparedFile>> {
    build_rewritten_files_at(&game_root(game)?, entries)
}

fn build_rewritten_files_at(root: &Path, entries: &[ContentEntry]) -> AppResult<Vec<PreparedFile>> {
    let desired: HashMap<&str, &str> = entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry.value.as_str()))
        .collect();
    let mut prepared = Vec::new();
    for relative in [SETTINGS_RELATIVE_PATH, PROFILE_RELATIVE_PATH] {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        if fs::metadata(&path)?.permissions().readonly() {
            return Err(AppError::other(format!(
                "配置文件为只读，无法应用：{}",
                path.display()
            )));
        }
        let bytes = fs::read(&path)?;
        let content = String::from_utf8(bytes)
            .map_err(|_| AppError::other(format!("配置文件不是有效 UTF-8：{}", path.display())))?;
        let mut rewritten = String::with_capacity(content.len());
        let mut changed = false;
        for line in content.split_inclusive('\n') {
            match parse_line(line, false) {
                Ok(Some(parsed)) if desired.contains_key(parsed.key) => {
                    let next = desired[parsed.key];
                    if parsed.value != next {
                        rewritten.push_str(&line[..parsed.value_start]);
                        rewritten.push_str(next);
                        rewritten.push_str(&line[parsed.value_end..]);
                        changed = true;
                    } else {
                        rewritten.push_str(line);
                    }
                }
                _ => rewritten.push_str(line),
            }
        }
        if changed {
            prepared.push(PreparedFile {
                path,
                new_bytes: Some(rewritten.into_bytes()),
            });
        }
    }
    Ok(prepared)
}

fn replace_files_transactionally(files: Vec<PreparedFile>) -> AppResult<()> {
    replace_files_transactionally_inner(files, None)
}

fn replace_files_transactionally_inner(
    files: Vec<PreparedFile>,
    fail_after_swaps: Option<usize>,
) -> AppResult<()> {
    let transaction_id = Uuid::new_v4().simple().to_string();
    let mut states = Vec::new();
    for (index, file) in files.into_iter().enumerate() {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(parent)?;
        }
        if file.path.exists() && fs::metadata(&file.path)?.permissions().readonly() {
            return Err(AppError::other(format!(
                "文件为只读，无法写入：{}",
                file.path.display()
            )));
        }
        let name = file
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config.cfg");
        let parent = file.path.parent().unwrap_or_else(|| Path::new("."));
        let temp = file
            .new_bytes
            .as_ref()
            .map(|_| parent.join(format!(".{name}.{transaction_id}.{index}.tmp")));
        let rollback = parent.join(format!(".{name}.{transaction_id}.{index}.rollback"));
        if let (Some(temp), Some(new_bytes)) = (&temp, &file.new_bytes) {
            fs::write(temp, new_bytes)?;
            if fs::read(temp)? != *new_bytes {
                let _ = fs::remove_file(temp);
                return Err(AppError::other("临时配置文件校验失败"));
            }
        }
        states.push((
            SwapState {
                target: file.path,
                temp,
                rollback: None,
                target_existed: false,
                touched: false,
            },
            file.new_bytes,
            rollback,
        ));
    }

    let result: AppResult<()> = (|| {
        for (index, (state, _, rollback_path)) in states.iter_mut().enumerate() {
            state.touched = true;
            state.target_existed = state.target.exists();
            if state.target_existed {
                fs::rename(&state.target, &*rollback_path)?;
                state.rollback = Some(rollback_path.clone());
            }
            if let Some(temp) = &state.temp {
                if let Err(error) = fs::rename(temp, &state.target) {
                    return Err(AppError::Io(error));
                }
            }
            if fail_after_swaps == Some(index + 1) {
                return Err(AppError::other("测试注入的事务失败"));
            }
        }
        for (state, expected, _) in &states {
            match expected {
                Some(expected) if fs::read(&state.target)? != *expected => {
                    return Err(AppError::other(format!(
                        "写入后校验失败：{}",
                        state.target.display()
                    )));
                }
                None if state.target.exists() => {
                    return Err(AppError::other(format!(
                        "删除后校验失败：{}",
                        state.target.display()
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    })();

    if let Err(error) = result {
        rollback_swaps(&states);
        return Err(error);
    }

    for (state, _, _) in states {
        if let Some(rollback) = state.rollback {
            let _ = fs::remove_file(rollback);
        }
        if let Some(temp) = state.temp {
            let _ = fs::remove_file(temp);
        }
    }
    Ok(())
}

fn rollback_swaps(states: &[(SwapState, Option<Vec<u8>>, PathBuf)]) {
    for (state, _, _) in states.iter().rev() {
        if !state.touched {
            if let Some(temp) = &state.temp {
                if temp.exists() {
                    let _ = fs::remove_file(temp);
                }
            }
            continue;
        }
        if let Some(rollback) = &state.rollback {
            if state.target.exists() {
                let _ = fs::remove_file(&state.target);
            }
            let _ = fs::rename(rollback, &state.target);
        } else if !state.target_existed && state.target.exists() {
            let _ = fs::remove_file(&state.target);
        }
        if let Some(temp) = &state.temp {
            if temp.exists() {
                let _ = fs::remove_file(temp);
            }
        }
    }
}

fn create_backup_internal(
    config_dir: &Path,
    game: ConfigGame,
    label: String,
    operation_source: String,
    allow_empty: bool,
) -> AppResult<ConfigBackupRecord> {
    let game_path = game_root(game)?;
    create_backup_at_root(
        config_dir,
        game,
        &game_path,
        label,
        operation_source,
        allow_empty,
    )
}

fn create_backup_at_root(
    config_dir: &Path,
    game: ConfigGame,
    game_path: &Path,
    label: String,
    operation_source: String,
    allow_empty: bool,
) -> AppResult<ConfigBackupRecord> {
    let created_at_ms = now_ms()?;
    let id = Uuid::new_v4().to_string();
    let backup_dir = backup_game_root(config_dir, game).join(format!("{created_at_ms}-{id}"));
    let files_dir = backup_dir.join("files");
    let mut files = Vec::new();
    let mut total_size = 0;
    for relative in [SETTINGS_RELATIVE_PATH, PROFILE_RELATIVE_PATH] {
        let source = game_path.join(relative);
        if !source.is_file() {
            continue;
        }
        let bytes = fs::read(&source)?;
        let destination = files_dir.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, &bytes)?;
        let size = bytes.len() as u64;
        total_size += size;
        files.push(BackupFileRecord {
            relative_path: relative.to_string(),
            sha256: hash_bytes(&bytes),
            size,
        });
    }
    if files.is_empty() && !allow_empty {
        return Err(AppError::NotFound(format!(
            "{} 没有可备份的配置文件",
            game.folder_name()
        )));
    }
    fs::create_dir_all(&backup_dir)?;
    let record = ConfigBackupRecord {
        id,
        game,
        created_at_ms,
        label,
        operation_source,
        files,
        total_size,
    };
    fs::write(
        backup_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&record)?,
    )?;
    Ok(record)
}

fn restore_record(record: &ConfigBackupRecord, backup_dir: &Path) -> AppResult<()> {
    let root = game_root(record.game)?;
    restore_record_at_root(record, backup_dir, &root)
}

fn restore_record_at_root(
    record: &ConfigBackupRecord,
    backup_dir: &Path,
    root: &Path,
) -> AppResult<()> {
    let mut prepared = Vec::new();
    let mut recorded_paths = HashSet::new();
    for file in &record.files {
        let relative = safe_relative_path(&file.relative_path)?;
        if relative != PathBuf::from(SETTINGS_RELATIVE_PATH)
            && relative != PathBuf::from(PROFILE_RELATIVE_PATH)
        {
            return Err(AppError::InvalidPath("备份包含不允许恢复的文件".into()));
        }
        if !recorded_paths.insert(relative.clone()) {
            return Err(AppError::InvalidPath("备份包含重复文件".into()));
        }
        let source = backup_dir.join("files").join(&relative);
        let bytes = fs::read(&source)?;
        if hash_bytes(&bytes) != file.sha256 {
            return Err(AppError::other(format!(
                "备份文件校验失败：{}",
                file.relative_path
            )));
        }
        prepared.push(PreparedFile {
            path: root.join(relative),
            new_bytes: Some(bytes),
        });
    }
    for relative in [SETTINGS_RELATIVE_PATH, PROFILE_RELATIVE_PATH] {
        let relative = PathBuf::from(relative);
        if !recorded_paths.contains(&relative) {
            prepared.push(PreparedFile {
                path: root.join(relative),
                new_bytes: None,
            });
        }
    }
    replace_files_transactionally(prepared)
}

fn find_backup(
    config_dir: &Path,
    backup_id: &str,
) -> AppResult<(ConfigBackupRecord, PathBuf, ConfigGame)> {
    if Uuid::parse_str(backup_id).is_err() {
        return Err(AppError::InvalidPath("备份 ID 无效".into()));
    }
    for game in [ConfigGame::Apex, ConfigGame::R5] {
        let root = backup_game_root(config_dir, game);
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let manifest_path = entry.path().join("manifest.json");
            let Ok(bytes) = fs::read(&manifest_path) else {
                continue;
            };
            let Ok(record) = serde_json::from_slice::<ConfigBackupRecord>(&bytes) else {
                continue;
            };
            if record.id == backup_id {
                if record.game != game {
                    return Err(AppError::InvalidPath("备份目标游戏不匹配".into()));
                }
                return Ok((record, entry.path(), game));
            }
        }
    }
    Err(AppError::NotFound("备份不存在".into()))
}

fn safe_relative_path(value: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(value.replace('\\', "/"));
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::InvalidPath("备份文件路径无效".into()));
    }
    Ok(path)
}

fn backup_game_root(config_dir: &Path, game: ConfigGame) -> PathBuf {
    config_dir
        .join("game-config-sync")
        .join("backups")
        .join(game.slug())
}

fn game_root(game: ConfigGame) -> AppResult<PathBuf> {
    Ok(saved_games_path()?.join("Respawn").join(game.folder_name()))
}

fn saved_games_path() -> AppResult<PathBuf> {
    #[cfg(windows)]
    {
        use known_folders::{get_known_folder_path, KnownFolder};
        if let Some(path) = get_known_folder_path(KnownFolder::SavedGames) {
            return Ok(path);
        }
    }
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|path| path.join("Saved Games"))
        .ok_or_else(|| AppError::NotFound("无法定位 Windows Saved Games 目录".into()))
}

fn ensure_game_not_running() -> AppResult<()> {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    if system.processes().values().any(|process| {
        process
            .name()
            .to_string_lossy()
            .eq_ignore_ascii_case("r5apex.exe")
    }) {
        return Err(AppError::other(
            "检测到 r5apex.exe 正在运行，请退出游戏后再修改配置",
        ));
    }
    Ok(())
}

fn ensure_game_files_writable(game: ConfigGame) -> AppResult<()> {
    let root = game_root(game)?;
    for relative in [SETTINGS_RELATIVE_PATH, PROFILE_RELATIVE_PATH] {
        let path = root.join(relative);
        if path.is_file() && fs::metadata(&path)?.permissions().readonly() {
            return Err(AppError::other(format!(
                "配置文件为只读，无法修改：{}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn unique_games(games: Vec<ConfigGame>) -> Vec<ConfigGame> {
    let mut seen = HashSet::new();
    games
        .into_iter()
        .filter(|game| seen.insert(*game))
        .collect()
}

fn canonical_content(entries: &[ContentEntry]) -> String {
    entries
        .iter()
        .map(|entry| format!("{} \"{}\"", entry.key, entry.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn now_ms() -> AppResult<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::other("系统时间早于 UNIX epoch"))?
        .as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn strict_content_rejects_unknown_and_duplicate_keys() {
        assert!(parse_content("fps_max \"144\"").is_err());
        assert!(parse_content("mouse_sensitivity \"1\"\nmouse_sensitivity \"2\"").is_err());
    }

    #[test]
    fn selected_content_keeps_only_individually_selected_fields() {
        let entries = selected_content_entries(
            "mouse_sensitivity \"1.2\"\ncl_fovScale \"1.55\"",
            &["cl_fovScale".to_string()],
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "cl_fovScale");
    }

    #[test]
    fn rewrite_changes_only_existing_values_and_preserves_format() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("Apex");
        fs::create_dir_all(root.join("local")).unwrap();
        fs::create_dir_all(root.join("profile")).unwrap();
        fs::write(
            root.join(SETTINGS_RELATIVE_PATH),
            b"\xef\xbb\xbfmouse_sensitivity   \"0.83\" // keep\r\nunknown \"x\"\r\nmouse_sensitivity \"9\"; quit\r\n",
        )
        .unwrap();
        let entries =
            parse_content("mouse_sensitivity \"1.25\"\nmouse_zoomed_sensitivity_scalar_0 \"1.1\"")
                .unwrap();
        let definitions: HashMap<&str, &str> = entries
            .iter()
            .map(|entry| (entry.key.as_str(), entry.value.as_str()))
            .collect();
        let content =
            String::from_utf8(fs::read(root.join(SETTINGS_RELATIVE_PATH)).unwrap()).unwrap();
        let mut rewritten = String::new();
        for line in content.split_inclusive('\n') {
            match parse_line(line, false) {
                Ok(Some(parsed)) if definitions.contains_key(parsed.key) => {
                    rewritten.push_str(&line[..parsed.value_start]);
                    rewritten.push_str(definitions[parsed.key]);
                    rewritten.push_str(&line[parsed.value_end..]);
                }
                _ => rewritten.push_str(line),
            }
        }
        assert_eq!(
            rewritten,
            "\u{feff}mouse_sensitivity   \"1.25\" // keep\r\nunknown \"x\"\r\nmouse_sensitivity \"9\"; quit\r\n"
        );
        assert!(!rewritten.contains("mouse_zoomed_sensitivity_scalar_0"));
    }

    #[test]
    fn scanner_finds_fields_across_both_config_files() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("local")).unwrap();
        fs::create_dir_all(dir.path().join("profile")).unwrap();
        fs::write(
            dir.path().join(SETTINGS_RELATIVE_PATH),
            "mouse_sensitivity \"0.83\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join(PROFILE_RELATIVE_PATH),
            "gamepad_aim_speed \"2\"\ncl_fovScale \"1.55\"\n",
        )
        .unwrap();
        let snapshot = scan_game_at(ConfigGame::Apex, dir.path().to_path_buf()).unwrap();
        assert_eq!(
            snapshot
                .entries
                .iter()
                .find(|entry| entry.key == "mouse_sensitivity")
                .and_then(|entry| entry.value.as_deref()),
            Some("0.83")
        );
        assert_eq!(
            snapshot
                .entries
                .iter()
                .find(|entry| entry.key == "gamepad_aim_speed")
                .and_then(|entry| entry.value.as_deref()),
            Some("2")
        );
    }

    #[test]
    fn scanner_marks_duplicate_different_values_as_conflict() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("local")).unwrap();
        fs::create_dir_all(dir.path().join("profile")).unwrap();
        fs::write(
            dir.path().join(SETTINGS_RELATIVE_PATH),
            "mouse_sensitivity \"0.8\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join(PROFILE_RELATIVE_PATH),
            "mouse_sensitivity \"1.2\"\n",
        )
        .unwrap();

        let snapshot = scan_game_at(ConfigGame::Apex, dir.path().to_path_buf()).unwrap();
        let entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.key == "mouse_sensitivity")
            .unwrap();
        assert!(entry.conflict);
        assert_eq!(entry.values, ["0.8", "1.2"]);
        assert!(generate_config_content_from_snapshot(
            ConfigGame::Apex,
            vec!["mouse_sensitivity".to_string()],
            snapshot.clone(),
        )
        .is_err());

        let desired = parse_content("mouse_sensitivity \"1.25\"").unwrap();
        let preview = analyze_target_snapshot(ConfigGame::Apex, &desired, snapshot).unwrap();
        assert_eq!(preview.replace_count, 1);
        assert_eq!(preview.conflict_count, 1);
    }

    #[test]
    fn rewrite_skips_missing_fields_and_replaces_all_existing_duplicates() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("local")).unwrap();
        fs::create_dir_all(dir.path().join("profile")).unwrap();
        fs::write(
            dir.path().join(SETTINGS_RELATIVE_PATH),
            "mouse_sensitivity \"0.8\"\nmouse_sensitivity \"1.0\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join(PROFILE_RELATIVE_PATH),
            "cl_fovScale \"1.4\"\n",
        )
        .unwrap();
        let entries =
            parse_content("mouse_sensitivity \"1.25\"\nmouse_zoomed_sensitivity_scalar_0 \"1.1\"")
                .unwrap();

        let prepared = build_rewritten_files_at(dir.path(), &entries).unwrap();
        assert_eq!(prepared.len(), 1);
        replace_files_transactionally(prepared).unwrap();
        let settings = fs::read_to_string(dir.path().join(SETTINGS_RELATIVE_PATH)).unwrap();
        let profile = fs::read_to_string(dir.path().join(PROFILE_RELATIVE_PATH)).unwrap();
        assert_eq!(
            settings,
            "mouse_sensitivity \"1.25\"\nmouse_sensitivity \"1.25\"\n"
        );
        assert_eq!(profile, "cl_fovScale \"1.4\"\n");
        assert!(!settings.contains("mouse_zoomed_sensitivity_scalar_0"));
    }

    #[test]
    fn multi_file_transaction_rolls_back_every_target_on_failure() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.cfg");
        let second = dir.path().join("second.cfg");
        fs::write(&first, b"first-before").unwrap();
        fs::write(&second, b"second-before").unwrap();

        let result = replace_files_transactionally_inner(
            vec![
                PreparedFile {
                    path: first.clone(),
                    new_bytes: Some(b"first-after".to_vec()),
                },
                PreparedFile {
                    path: second.clone(),
                    new_bytes: Some(b"second-after".to_vec()),
                },
            ],
            Some(1),
        );

        assert!(result.is_err());
        assert_eq!(fs::read(first).unwrap(), b"first-before");
        assert_eq!(fs::read(second).unwrap(), b"second-before");
    }

    #[test]
    fn complete_backup_restores_original_files_and_hashes() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join("launcher");
        let game_dir = dir.path().join("game");
        fs::create_dir_all(game_dir.join("local")).unwrap();
        fs::create_dir_all(game_dir.join("profile")).unwrap();
        let settings_before = b"mouse_sensitivity \"0.83\"\r\n";
        let profile_before = b"cl_fovScale \"1.55\"\n";
        fs::write(game_dir.join(SETTINGS_RELATIVE_PATH), settings_before).unwrap();
        fs::write(game_dir.join(PROFILE_RELATIVE_PATH), profile_before).unwrap();

        let record = create_backup_at_root(
            &config_dir,
            ConfigGame::Apex,
            &game_dir,
            "unit test".to_string(),
            "test".to_string(),
            false,
        )
        .unwrap();
        let backup_dir = backup_game_root(&config_dir, ConfigGame::Apex)
            .join(format!("{}-{}", record.created_at_ms, record.id));
        fs::write(game_dir.join(SETTINGS_RELATIVE_PATH), b"changed").unwrap();
        fs::write(game_dir.join(PROFILE_RELATIVE_PATH), b"changed too").unwrap();

        restore_record_at_root(&record, &backup_dir, &game_dir).unwrap();
        assert_eq!(
            fs::read(game_dir.join(SETTINGS_RELATIVE_PATH)).unwrap(),
            settings_before
        );
        assert_eq!(
            fs::read(game_dir.join(PROFILE_RELATIVE_PATH)).unwrap(),
            profile_before
        );
        assert_eq!(record.files[0].sha256, hash_bytes(settings_before));
        assert_eq!(record.files[1].sha256, hash_bytes(profile_before));
    }

    #[test]
    fn restoring_snapshot_also_restores_file_absence() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join("launcher");
        let game_dir = dir.path().join("game");
        fs::create_dir_all(game_dir.join("local")).unwrap();
        fs::create_dir_all(game_dir.join("profile")).unwrap();
        fs::write(
            game_dir.join(PROFILE_RELATIVE_PATH),
            b"cl_fovScale \"1.55\"\n",
        )
        .unwrap();
        let record = create_backup_at_root(
            &config_dir,
            ConfigGame::Apex,
            &game_dir,
            "without settings".to_string(),
            "test".to_string(),
            false,
        )
        .unwrap();
        let backup_dir = backup_game_root(&config_dir, ConfigGame::Apex)
            .join(format!("{}-{}", record.created_at_ms, record.id));
        fs::write(
            game_dir.join(SETTINGS_RELATIVE_PATH),
            b"mouse_sensitivity \"1\"\n",
        )
        .unwrap();

        restore_record_at_root(&record, &backup_dir, &game_dir).unwrap();

        assert!(!game_dir.join(SETTINGS_RELATIVE_PATH).exists());
        assert!(game_dir.join(PROFILE_RELATIVE_PATH).exists());
    }
}
