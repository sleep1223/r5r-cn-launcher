import { invoke } from "./invoke";
import {
  DiagnosticReportResult,
  DiskSuggestion,
  LauncherSettings,
  PathValidation,
} from "./types";

export const loadSettings = () => invoke<LauncherSettings>("load_settings");

export const saveSettings = (settings: LauncherSettings) =>
  invoke<void>("save_settings", { settings });

export const validateInstallPath = (path: string) =>
  invoke<PathValidation>("validate_install_path", { path });

export const openLogFolder = () => invoke<void>("open_log_folder");

export const openConfigFolder = () => invoke<void>("open_config_folder");

export const openExternalUrl = (url: string) =>
  invoke<void>("open_external_url", { url });

export const collectCrashDiagnostics = (destination: string) =>
  invoke<DiagnosticReportResult>("collect_crash_diagnostics", { destination });

export const openDiagnosticReportFolder = (path: string) =>
  invoke<void>("open_diagnostic_report_folder", { path });

export const suggestInstallPath = () =>
  invoke<DiskSuggestion[]>("suggest_install_path");
