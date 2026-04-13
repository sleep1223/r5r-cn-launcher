// API client for r5.sleep0.de community services.
// All endpoints return { code, data, msg } — we unwrap `data` here.

const BASE = "https://r5.sleep0.de/api";

async function get<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, { headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  return json?.data ?? json;
}

async function post<T>(path: string, headers?: Record<string, string>): Promise<T> {
  const resp = await fetch(`${BASE}${path}`, { method: "POST", headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  return json?.data ?? json;
}

// ===== Types =====

export interface PlayerInServer {
  name: string;
  country?: string;
  region?: string | null;
}

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
  kills: number;
  deaths: number;
  kd: number;
}

export interface WeaponLeaderboardRecord {
  weapon: string;
  name: string;
  nucleus_id?: number;
  kills: number;
  deaths: number;
  kd: number;
}

export interface WeaponRecord {
  weapon: string;
  kills: number;
  deaths: number;
  kd: number;
}

export interface PlayerVsAllRecord {
  opponent_name: string;
  opponent_id?: number;
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

// ===== Endpoints =====

export const getServers = () => get<ServerListItem[]>("/v1/r5/server");

type LeaderboardParams = {
  range?: "today" | "yesterday" | "week" | "month" | "all";
  page_no?: number;
  page_size?: number;
  sort?: "kills" | "deaths" | "kd";
};

function qs(params: Record<string, string | number | undefined>): string {
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
  p?: { page_no?: number; page_size?: number; sort?: string },
) =>
  get<{ data: PlayerVsAllRecord[]; summary?: PlayerVsAllSummary; total?: number }>(
    `/v1/r5/players/${encodeURIComponent(name)}/vs_all${qs({ ...p })}`,
  );

export const getPlayerWeapons = (
  name: string,
  p?: { page_no?: number; page_size?: number; sort?: string },
) => get<WeaponRecord[]>(`/v1/r5/players/${encodeURIComponent(name)}/weapons${qs({ ...p })}`);

export const getUserMe = (appKey: string) =>
  get<UserInfo>("/v1/r5/user/me", { "X-App-Key": appKey });

export const getTeams = (p?: { page_no?: number; page_size?: number }) =>
  get<{ data: Team[]; total?: number }>(`/v1/r5/teams${qs({ ...p })}`);

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
