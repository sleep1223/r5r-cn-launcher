import { invoke } from "./invoke";
import { LaunchOptionSelection } from "./types";

export const launchGame = (
  channel: string,
  selection: LaunchOptionSelection,
  installDirOverride?: string,
) =>
  invoke<number>("launch_game_cmd", {
    channel,
    selection,
    installDirOverride: installDirOverride ?? null,
  });
