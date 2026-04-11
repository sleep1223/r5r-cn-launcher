import { invoke } from "./invoke";
import { OfflineSource } from "./types";

export const startOfflineImport = (
  installRoot: string,
  source: OfflineSource,
  verifyAfter = false,
) =>
  invoke<string>("start_offline_import", {
    installRoot,
    source,
    verifyAfter,
  });

export const startOnlineInstall = (channel: string) =>
  invoke<string>("start_online_install", { channel });

export const startUpdate = (channel: string) =>
  invoke<string>("start_update", { channel });

export const startRepair = (channel: string) =>
  invoke<string>("start_repair", { channel });

export interface UpdateStatus {
  has_update: boolean;
  local_version: string | null;
  remote_version: string;
}

export const checkUpdate = (channel: string) =>
  invoke<UpdateStatus>("check_update", { channel });

export const cancelInstall = (jobId: string) =>
  invoke<boolean>("cancel_install", { jobId });

export const pauseInstall = (jobId: string, paused: boolean) =>
  invoke<boolean>("pause_install", { jobId, paused });
