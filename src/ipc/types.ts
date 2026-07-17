// TypeScript mirrors of the Rust IPC types. Keep in sync with src-tauri/src/.

export type ProxyMode =
  | { kind: "system" }
  | { kind: "custom"; url: string }
  | { kind: "none" };

export interface PerChannelState {
  installed: boolean;
  version: string;
  key: string;
  installed_languages: string[];
}

export type UpdateStrategy = "verify" | "patch";

export interface LauncherSettings {
  schema_version: number;
  proxy_mode: ProxyMode;
  mirror_domain: string;
  library_root: string;
  selected_channel: string;
  concurrent_downloads: number;
  channels: Record<string, PerChannelState>;
  launch_option_selection: unknown;
  last_known_official_install_path: string | null;
  update_strategy: UpdateStrategy;
  download_hd_textures: boolean;
  launch_via_ea_app: boolean;
}

// ===== Community dashboard =====

export interface DashboardPatch {
  from_version: string;
  to_version: string;
  url: string;
}

export interface DashboardAnnouncement {
  title: string;
  content: string;
}

export interface DashboardRule {
  icon: string;
  text: string;
}

export interface DashboardConfig {
  offline_package_url: string;
  download_domain: string;
  docs_url: string;
  launcher_version: string;
  launcher_update_url: string;
  force_update: boolean;
  game_version: string;
  patches: DashboardPatch[];
  announcement: DashboardAnnouncement;
  rules: DashboardRule[];
}

export interface DiskSuggestion {
  path: string;
  free_bytes: number;
}

export interface PathValidation {
  ok: boolean;
  normalized: string;
  errors: string[];
  warnings: string[];
}

export interface ProxyTestResult {
  ok: boolean;
  status: number | null;
  latency_ms: number;
  error: string | null;
}

export type DetectSource = "shortcut" | "registry" | "library_scan";

export interface DetectedInstall {
  source: DetectSource;
  path: string;
  channel: string | null;
  version: string | null;
  /** True when `<path>/r5apex.exe` exists — safe to launch directly from `path`. */
  has_game: boolean;
}

// ===== Launch options =====

export interface EnumArgChoice {
  value: string;
  label_zh: string;
  args: string[];
}

export type OptionKind =
  | { type: "toggle"; args: string[]; is_combo: boolean }
  | { type: "int"; flag: string; min: number; max: number }
  | { type: "float"; flag: string; min: number; max: number; step: number }
  | { type: "int_pair"; x_flag: string; y_flag: string }
  | { type: "enum"; flag: string; choices: [string, string][] }
  | { type: "enum_args"; choices: EnumArgChoice[] }
  | { type: "fov_degrees"; flag: string; min: number; max: number; base: number }
  | { type: "string"; flag: string; placeholder: string };

export type OptionValue =
  | { type: "bool"; value: boolean }
  | { type: "int"; value: number }
  | { type: "float"; value: number }
  | { type: "int_pair"; value: [number, number] }
  | { type: "enum"; value: string }
  | { type: "string"; value: string };

export type RiskLevel = "none" | "caution" | "danger";

export interface OptionEntry {
  id: string;
  category: string;
  kind: OptionKind;
  default_enabled: boolean;
  default_value: OptionValue | null;
  label_zh: string;
  description_zh: string;
  risk: RiskLevel;
  conflicts_with: string[];
}

export interface Category {
  id: string;
  label_zh: string;
}

export interface LaunchOptionCatalog {
  categories: Category[];
  entries: OptionEntry[];
}

export interface SelectionEntry {
  enabled: boolean;
  value: OptionValue | null;
}

export interface LaunchOptionSelection {
  items: Record<string, SelectionEntry>;
}

export type WarningSeverity = "info" | "caution" | "danger";

export interface LaunchWarning {
  severity: WarningSeverity;
  message_zh: string;
  related_option_ids: string[];
}

export interface LaunchExitedEvent {
  pid: number;
  code: number | null;
  success: boolean;
}

// ===== Apex / R5 config sync =====

export type ConfigGame = "apex" | "r5";
export type ConfigSection = "mouse_keyboard" | "controller" | "fov";
export type PreviewStatus = "replace" | "unchanged" | "missing";

export interface ConfigEntryState {
  key: string;
  label: string;
  section: ConfigSection;
  present: boolean;
  value: string | null;
  values: string[];
  locations: string[];
  conflict: boolean;
}

export interface ConfigFileFingerprint {
  path: string;
  sha256: string;
  size: number;
}

export interface GameConfigSnapshot {
  game: ConfigGame;
  detected: boolean;
  root_path: string;
  files: ConfigFileFingerprint[];
  entries: ConfigEntryState[];
}

export interface ConfigComparison {
  apex: GameConfigSnapshot;
  r5: GameConfigSnapshot;
}

export interface GeneratedConfigContent {
  source_game: ConfigGame;
  content: string;
  keys: string[];
  has_mouse: boolean;
  has_controller: boolean;
  has_fov: boolean;
}

export interface ApplyPreviewItem {
  key: string;
  label: string;
  section: ConfigSection;
  desired: string;
  current_values: string[];
  status: PreviewStatus;
}

export interface TargetApplyPreview {
  game: ConfigGame;
  files: ConfigFileFingerprint[];
  replace_count: number;
  unchanged_count: number;
  missing_count: number;
  conflict_count: number;
  items: ApplyPreviewItem[];
}

export interface ConfigApplyPreview {
  content_sha256: string;
  selected_keys: string[];
  targets: TargetApplyPreview[];
}

export interface ApplyConfigRequest {
  content: string;
  selected_keys: string[];
  target_games: ConfigGame[];
  expected_preview: ConfigApplyPreview;
  source_label: string;
}

export interface ApplyTargetResult {
  game: ConfigGame;
  replaced: number;
  unchanged: number;
  missing: number;
  backup_id: string | null;
}

export interface ConfigApplyResult {
  targets: ApplyTargetResult[];
}

export interface BackupFileRecord {
  relative_path: string;
  sha256: string;
  size: number;
}

export interface ConfigBackupRecord {
  id: string;
  game: ConfigGame;
  created_at_ms: number;
  label: string;
  operation_source: string;
  files: BackupFileRecord[];
  total_size: number;
}

export type InstallPhase =
  | { phase: "preparing" }
  | { phase: "fetching_manifest" }
  | { phase: "scanning" }
  | { phase: "downloading" }
  | { phase: "merging_parts" }
  | { phase: "verifying" }
  | { phase: "complete" }
  | { phase: "failed"; reason: string }
  | { phase: "cancelled" };

export type InstallLogLevel = "info" | "warn" | "error";

export interface InstallLogEvent {
  job_id: string;
  ts_ms: number;
  level: InstallLogLevel;
  message: string;
}

export interface ProgressEvent {
  job_id: string;
  phase: InstallPhase;
  file_index: number;
  file_count: number;
  bytes_done: number;
  bytes_total: number;
  current_file: string;
  speed_bps: number;
  eta_seconds: number;
}

export type OfflineSource =
  | { type: "directory"; path: string }
  | { type: "zip"; path: string };

export interface AppErrorPayload {
  kind: string;
  message: string;
}

export class AppError extends Error {
  kind: string;
  constructor(payload: AppErrorPayload) {
    super(payload.message);
    this.kind = payload.kind;
  }
}
