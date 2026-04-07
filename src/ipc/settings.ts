import { invoke } from "./invoke";
import { LauncherSettings, PathValidation } from "./types";

export const loadSettings = () => invoke<LauncherSettings>("load_settings");

export const saveSettings = (settings: LauncherSettings) =>
  invoke<void>("save_settings", { settings });

export const validateInstallPath = (path: string) =>
  invoke<PathValidation>("validate_install_path", { path });

export const openLogFolder = () => invoke<void>("open_log_folder");
