import { invoke } from "./invoke";
import { LaunchOptionCatalog, LaunchOptionSelection, LaunchWarning } from "./types";

export const getLaunchOptionCatalog = () =>
  invoke<LaunchOptionCatalog>("get_launch_option_catalog");

export const validateLaunchArgs = (selection: LaunchOptionSelection) =>
  invoke<LaunchWarning[]>("validate_launch_args_cmd", { selection });

export const composeLaunchArgs = (selection: LaunchOptionSelection) =>
  invoke<string[]>("compose_launch_args_cmd", { selection });
