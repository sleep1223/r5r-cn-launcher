import { invoke } from "./invoke";
import { DiskSuggestion, LauncherSettings, PathValidation } from "./types";

export const loadSettings = () => invoke<LauncherSettings>("load_settings");

export const saveSettings = (settings: LauncherSettings) =>
  invoke<void>("save_settings", { settings });

export const validateInstallPath = (path: string) =>
  invoke<PathValidation>("validate_install_path", { path });

export const openLogFolder = () => invoke<void>("open_log_folder");

export const openConfigFolder = () => invoke<void>("open_config_folder");

export const openExternalUrl = (url: string) =>
  invoke<void>("open_external_url", { url });

export const suggestInstallPath = () =>
  invoke<DiskSuggestion[]>("suggest_install_path");
