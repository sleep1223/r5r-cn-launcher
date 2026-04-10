import { invoke } from "./invoke";

export interface VersionInfo {
  current: string;
}

export const getLauncherVersion = () =>
  invoke<VersionInfo>("get_launcher_version");

export const downloadAndApplyUpdate = (url: string) =>
  invoke<void>("download_and_apply_update", { url });
