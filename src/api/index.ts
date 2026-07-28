// API client for r5.sleep0.de community services.
// All endpoints return { code, data, msg } — we unwrap `data` here.

const BASE = "https://r5.sleep0.de/api";

async function get<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, { headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  if (json?.code !== undefined && json.code !== "0000" && json.code !== 0) {
    throw new Error(json?.msg || `业务错误 ${json.code}`);
  }
  return json && Object.prototype.hasOwnProperty.call(json, "data")
    ? json.data
    : json;
}

/** Like get() but returns the full envelope (for endpoints where summary/total sit alongside data). */
async function getFull<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, { headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  if (json?.code !== undefined && json.code !== "0000" && json.code !== 0) {
    throw new Error(json?.msg || `业务错误 ${json.code}`);
  }
  return json;
}

async function post<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, { method: "POST", headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  return json?.data ?? json;
}

async function jsonRequest<T>(
  path: string,
  init: RequestInit,
): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, init);
  const json = await resp.json().catch(() => null);
  if (
    !resp.ok ||
    (json?.code !== undefined && json.code !== "0000" && json.code !== 0)
  ) {
    throw new Error(json?.msg || `HTTP ${resp.status}`);
  }
  return json && Object.prototype.hasOwnProperty.call(json, "data")
    ? json.data
    : json;
}

// ===== Types =====

export interface PlayerInServer {
  name: string;
  country?: string;
  region?: string | null;
}

export type InputDevice = "controller" | "keyboard_mouse" | "unknown";

export type StatsTimeRange =
  | "today"
  | "yesterday"
  | "week"
  | "last_week"
  | "month"
  | "all";

export interface ServerListItem {
  name: string;
  short_name?: string;
  full_name?: string;
  ip?: string | null;
  port?: number | null;
  region?: string | null;
  map?: string | null;
  playlist?: string | null;
  player_count: number;
  max_players?: number;
  has_status: boolean;
  ping?: number;
  country?: string | null;
  host?: string | null;
  players?: PlayerInServer[];
}

export interface KdRecord {
  name: string;
  nucleus_id?: number;
  input_device?: InputDevice | null;
  kills: number;
  deaths: number;
  kd: number;
}

export interface WeaponLeaderboardRecord {
  weapon: string;
  name: string;
  nucleus_id?: number;
  input_device?: InputDevice | null;
  kills: number;
  deaths: number;
  kd: number;
}

export interface WeaponRecord {
  weapon: string;
  input_device?: InputDevice | null;
  kills: number;
  deaths: number;
  kd: number;
}

export interface PlayerVsAllRecord {
  opponent_name: string;
  opponent_id?: number;
  input_device?: InputDevice | null;
  kills: number;
  deaths: number;
  kd: number;
}

export interface PlayerVsAllSummary {
  total_kills: number;
  total_deaths: number;
  kd: number;
  nemesis?: PlayerVsAllRecord;
  worst_enemy?: PlayerVsAllRecord;
}

export interface UserInfo {
  id: number;
  platform?: string;
  platform_uid?: string;
  player_id: number;
  player_name: string;
  nucleus_id?: number | null;
}

export interface MemberInfo {
  binding_id: number;
  platform?: string;
  platform_uid?: string;
  player_id: number;
  player_name: string;
  kd: number;
  role?: "creator" | "member";
  joined_at?: string | null;
}

export interface Team {
  id: number;
  creator: MemberInfo;
  slots_needed: number;
  slots_remaining: number;
  status: "open" | "full" | "cancelled" | "expired";
  members: MemberInfo[];
  created_at: string;
}

export interface TeamJoinResult {
  team: Team;
  notify_members: MemberInfo[] | null;
}

export type GameConfigSourceGame = "apex" | "r5";
export type GameConfigInputDevice = "mouse_keyboard" | "controller";

export interface GameConfigPresetSummary {
  id: number;
  creator_name: string;
  name: string;
  remark: string | null;
  source_game: GameConfigSourceGame;
  has_mouse: boolean;
  has_controller: boolean;
  has_fov: boolean;
  schema_version: number;
  created_at: string;
  updated_at: string;
}

export interface GameConfigPreset extends GameConfigPresetSummary {
  content: string;
}

export interface GameConfigPresetPage {
  data: GameConfigPresetSummary[];
  total: number;
  page_no?: number;
  page_size?: number;
}

export interface SaveGameConfigPreset {
  name: string;
  remark?: string | null;
  source_game: GameConfigSourceGame;
  content: string;
}

export type ApexPlatform = "PC" | "PS4" | "X1" | "SWITCH";

export interface ApexPlayerSummary {
  uid?: string | number | null;
  name?: string | null;
  platform?: ApexPlatform | string | null;
  level?: number | null;
  rank_score?: number | null;
  rank_name?: string | null;
  rank_name_zh?: string | null;
  rank_div?: number | null;
  rank_img?: string | null;
  selected_legend?: string | null;
  selected_legend_zh?: string | null;
  lobby_state?: string | null;
  lobby_state_zh?: string | null;
  is_online?: boolean | string | null;
  can_join?: boolean | string | null;
  party_full?: boolean | string | null;
  current_state?: string | null;
  current_state_zh?: string | null;
  current_state_text?: string | null;
  current_state_text_zh?: string | null;
}

export interface ApexPlayerComparison {
  has_previous: boolean;
  changes: Record<string, unknown>;
}

export interface ApexPlayerStats {
  resolved?: Record<string, unknown> | null;
  summary: ApexPlayerSummary;
  comparison?: ApexPlayerComparison;
  snapshot_id?: number | null;
  credit?: string;
}

export interface ApexCachePayload<T> {
  data: T;
  updated_at?: string | null;
  error?: string | null;
  credit?: string;
}

export interface ApexMapEntry {
  map?: string;
  map_zh?: string;
  remainingTimer?: string;
  remainingSecs?: number;
  asset?: string;
  eventName?: string;
  eventName_zh?: string;
}

export interface ApexMapMode {
  name?: string;
  name_zh?: string;
  current?: ApexMapEntry;
  next?: ApexMapEntry;
}

export interface ApexMapRotation {
  battle_royale?: ApexMapMode;
  ranked?: ApexMapMode;
  ltm?: ApexMapMode;
  wildcard?: ApexMapMode;
}

export interface ApexServerStatusRow {
  name: string;
  name_zh?: string;
  key?: string;
  status?: string;
  status_zh?: string;
  response_time?: number;
}

export interface ApexServerStatusSection {
  section_name: string;
  section_name_zh?: string;
  section_key: string;
  rows: ApexServerStatusRow[];
}

export interface ApexPredatorPlatform {
  name: string;
  val?: number;
  total_masters?: number;
}

export type ApexPredator = Record<string, ApexPredatorPlatform>;
export type ApexTranslations = Record<string, Record<string, string>>;

// ===== Endpoints =====

export const getServers = () => get<ServerListItem[]>("/v1/r5/server");

export const getApexPlayer = (params: {
  player_name?: string;
  uid?: string;
  platform?: ApexPlatform;
  resolve_uid_first?: boolean;
  save_snapshot?: boolean;
}) => get<ApexPlayerStats>(`/v1/r5/apex/player${qs(params)}`);

export const getApexTranslations = () =>
  get<ApexTranslations>("/v1/r5/apex/translations");

export const getApexMapRotation = () =>
  get<ApexCachePayload<ApexMapRotation>>("/v1/r5/apex/map-rotation");

export const getApexServerStatus = () =>
  get<ApexCachePayload<ApexServerStatusSection[]>>(
    "/v1/r5/apex/server-status",
  );

export const getApexPredator = () =>
  get<ApexCachePayload<ApexPredator>>("/v1/r5/apex/predator");

type LeaderboardParams = {
  range?: "today" | "yesterday" | "week" | "month" | "all";
  page_no?: number;
  page_size?: number;
  sort?: "kills" | "deaths" | "kd";
};

function qs(
  params: Record<string, string | number | boolean | undefined>,
): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined) q.set(k, String(v));
  }
  const s = q.toString();
  return s ? `?${s}` : "";
}

export const getKillLeaderboard = (p?: LeaderboardParams) =>
  get<KdRecord[]>(`/v1/r5/leaderboard/kd${qs({ ...p })}`);

export const getWeaponLeaderboard = (p?: LeaderboardParams) =>
  get<WeaponLeaderboardRecord[]>(`/v1/r5/leaderboard/weapon${qs({ ...p })}`);

export const getPlayerVsAll = (
  name: string,
  p?: {
    page_no?: number;
    page_size?: number;
    sort?: string;
    range?: StatsTimeRange;
  },
) =>
  getFull<{ data: PlayerVsAllRecord[]; summary?: PlayerVsAllSummary; total?: number }>(
    `/v1/r5/players/${encodeURIComponent(name)}/vs_all${qs({ ...p })}`,
  );

export const getPlayerWeapons = (
  name: string,
  p?: {
    page_no?: number;
    page_size?: number;
    sort?: string;
    range?: StatsTimeRange;
  },
) => get<WeaponRecord[]>(`/v1/r5/players/${encodeURIComponent(name)}/weapons${qs({ ...p })}`);

export const getUserMe = (appKey: string) =>
  get<UserInfo>("/v1/r5/user/me", { "X-App-Key": appKey });

export const getTeams = (p?: { page_no?: number; page_size?: number }) =>
  getFull<{ data: Team[]; total?: number }>(`/v1/r5/teams${qs({ ...p })}`);

export const createTeam = (appKey: string, slotsNeeded: number) =>
  post<Team>(`/v1/r5/teams/app/create?slots_needed=${slotsNeeded}`, {
    "X-App-Key": appKey,
  });

export const joinTeam = (appKey: string, teamId: number) =>
  post<TeamJoinResult>(`/v1/r5/teams/app/${teamId}/join`, {
    "X-App-Key": appKey,
  });

export const cancelTeam = (appKey: string, teamId: number) =>
  post<null>(`/v1/r5/teams/app/${teamId}/cancel`, { "X-App-Key": appKey });

export const leaveTeam = (appKey: string, teamId: number) =>
  post<null>(`/v1/r5/teams/app/${teamId}/leave`, { "X-App-Key": appKey });

export const getGameConfigPresets = (p?: {
  q?: string;
  input_device?: GameConfigInputDevice;
  page_no?: number;
  page_size?: number;
}) =>
  getFull<GameConfigPresetPage>(
    `/v1/r5/launcher/game-configs${qs({ ...p })}`,
  );

export const getGameConfigPreset = (id: number) =>
  get<GameConfigPreset>(`/v1/r5/launcher/game-configs/${id}`);

export const getMyGameConfigPreset = (appKey: string) =>
  get<GameConfigPreset>("/v1/r5/launcher/game-configs/mine", {
    "X-App-Key": appKey,
  });

export const saveMyGameConfigPreset = (
  appKey: string,
  payload: SaveGameConfigPreset,
) =>
  jsonRequest<GameConfigPreset>("/v1/r5/launcher/game-configs/mine", {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
      "X-App-Key": appKey,
    },
    body: JSON.stringify(payload),
  });

export const deleteMyGameConfigPreset = (appKey: string) =>
  jsonRequest<null>("/v1/r5/launcher/game-configs/mine", {
    method: "DELETE",
    headers: { "X-App-Key": appKey },
  });
