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

export interface LauncherSettings {
  schema_version: number;
  proxy_mode: ProxyMode;
  root_config_url: string;
  library_root: string;
  selected_channel: string;
  concurrent_downloads: number;
  channels: Record<string, PerChannelState>;
  launch_option_selection: unknown;
  last_known_official_install_path: string | null;
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
}

export interface RemoteChannel {
  name: string;
  game_url: string;
  dedi_url: string;
  enabled: boolean;
  requires_key: boolean;
  allow_updates: boolean;
  key: string;
}

export interface RemoteConfig {
  launcher_version: string;
  updater_version: string;
  self_updater: string;
  background_video: string;
  allow_updates: boolean;
  force_updates: boolean;
  channels: RemoteChannel[];
}

// ===== Launch options =====

export type OptionKind =
  | { type: "toggle"; args: string[] }
  | { type: "int"; flag: string; min: number; max: number }
  | { type: "float"; flag: string; min: number; max: number; step: number }
  | { type: "int_pair"; x_flag: string; y_flag: string }
  | { type: "enum"; flag: string; choices: [string, string][] }
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

export type InstallPhase =
  | { phase: "preparing" }
  | { phase: "downloading" }
  | { phase: "merging_parts" }
  | { phase: "verifying" }
  | { phase: "complete" }
  | { phase: "failed"; reason: string }
  | { phase: "cancelled" };

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
