import { invoke } from "./invoke";

export type MatchmakingFixState =
  | "not_applicable"
  | "unfixed"
  | "fixed"
  | "file_missing"
  | "unexpected_content";

export interface MatchmakingFixStatus {
  state: MatchmakingFixState;
  game_version: string | null;
  affected_version: string;
  file_path: string | null;
  can_fix: boolean;
  can_restore: boolean;
}

export const checkMatchmakingFix = (channel: string) =>
  invoke<MatchmakingFixStatus>("check_matchmaking_fix", { channel });

export const applyMatchmakingFix = (channel: string) =>
  invoke<MatchmakingFixStatus>("apply_matchmaking_fix", { channel });

export const restoreMatchmakingFix = (channel: string) =>
  invoke<MatchmakingFixStatus>("restore_matchmaking_fix", { channel });
