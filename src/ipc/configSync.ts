import { invoke } from "./invoke";
import {
  ApplyConfigRequest,
  ConfigApplyPreview,
  ConfigApplyResult,
  ConfigBackupRecord,
  ConfigComparison,
  ConfigGame,
  GeneratedConfigContent,
} from "./types";

export const scanGameConfigs = () =>
  invoke<ConfigComparison>("scan_game_configs");

export const generateGameConfigContent = (
  sourceGame: ConfigGame,
  selectedKeys: string[],
) =>
  invoke<GeneratedConfigContent>("generate_game_config_content", {
    sourceGame,
    selectedKeys,
  });

export const previewGameConfigApply = (
  content: string,
  selectedKeys: string[],
  targetGames: ConfigGame[],
) =>
  invoke<ConfigApplyPreview>("preview_game_config_apply", {
    content,
    selectedKeys,
    targetGames,
  });

export const applyGameConfig = (request: ApplyConfigRequest) =>
  invoke<ConfigApplyResult>("apply_game_config", { request });

export const createGameConfigBackup = (game: ConfigGame, label = "手动备份") =>
  invoke<ConfigBackupRecord>("create_game_config_backup", { game, label });

export const listGameConfigBackups = (game?: ConfigGame) =>
  invoke<ConfigBackupRecord[]>("list_game_config_backups", { game });

export const deleteGameConfigBackup = (backupId: string) =>
  invoke<void>("delete_game_config_backup", { backupId });

export const restoreGameConfigBackup = (backupId: string) =>
  invoke<ConfigBackupRecord>("restore_game_config_backup", { backupId });

export const restoreLatestGameConfigBackup = (game: ConfigGame) =>
  invoke<ConfigBackupRecord>("restore_latest_game_config_backup", { game });
