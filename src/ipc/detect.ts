import { invoke } from "./invoke";
import { DetectedInstall } from "./types";

export const detectExistingR5R = () =>
  invoke<DetectedInstall[]>("detect_existing_r5r");

export interface AdoptResult {
  adopted: boolean;
  channel_dir: string | null;
  library_root: string | null;
  game_version: string | null;
  was_already_adopted: boolean;
}

export const autoAdoptExistingInstall = () =>
  invoke<AdoptResult>("auto_adopt_existing_install");
