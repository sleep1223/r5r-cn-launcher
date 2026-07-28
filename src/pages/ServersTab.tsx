import { useEffect, useRef, useState } from "react";
import { GlassCard } from "../components/GlassCard";
import { getServers, PlayerInServer, ServerListItem } from "../api";
import {
  countryName,
  flagImage,
  gamemodeName,
  mapImage,
  mapName,
} from "../utils/maps";
import { pingHost, PingResult } from "../ipc/ping";

type PingState = { status: "pending" } | { status: "done"; result: PingResult };

const serverKey = (sv: ServerListItem) =>
  [sv.short_name, sv.name, sv.map, sv.region, sv.country, sv.ip, sv.port]
    .filter((value) => value !== null && value !== undefined)
    .join("|");
const serverAddr = (sv: ServerListItem): { ip: string; port: number } | null => {
  const ip = (sv.ip ?? "").trim();
  const port = sv.port ?? 0;
  if (!ip || !port) return null;
  return { ip, port };
};

function pingColor(ms: number) {
  if (ms < 80) return "text-emerald-300";
  if (ms < 160) return "text-yellow-300";
  return "text-red-300";
}

interface PlayerCountryGroup {
  key: string;
  code: string | null;
  label: string;
  players: PlayerInServer[];
}

function groupPlayersByCountry(players: PlayerInServer[]): PlayerCountryGroup[] {
  const groups = new Map<string, PlayerInServer[]>();
  for (const player of players) {
    const code = player.country?.trim().toUpperCase() || "__UNKNOWN__";
    const group = groups.get(code) ?? [];
    group.push(player);
    groups.set(code, group);
  }

  return [...groups.entries()]
    .map(([key, groupedPlayers]) => {
      const code = key === "__UNKNOWN__" ? null : key;
      return {
        key,
        code,
        label: code ? countryName(code) || code : "未知国家",
        players: groupedPlayers.sort((a, b) => a.name.localeCompare(b.name)),
      };
    })
    .sort((a, b) => {
      if (a.code === null) return 1;
      if (b.code === null) return -1;
      return b.players.length - a.players.length || a.label.localeCompare(b.label);
    });
}

export function ServersTab() {
  const [servers, setServers] = useState<ServerListItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pings, setPings] = useState<Record<string, PingState>>({});
  const [expandedServers, setExpandedServers] = useState<Set<string>>(
    () => new Set(),
  );
  const pingSeqRef = useRef(0);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const data = await getServers();
        if (!cancelled) setServers(data);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    };
    load();
    const t = setInterval(load, 30_000);
    return () => { cancelled = true; clearInterval(t); };
  }, []);

  // Re-ping whenever the server list changes. Cap concurrency at 8.
  useEffect(() => {
    if (!servers) return;
    const seq = ++pingSeqRef.current;
    const targets = servers
      .map((sv) => ({ key: serverKey(sv), addr: serverAddr(sv) }))
      .filter((t): t is { key: string; addr: { ip: string; port: number } } => t.addr !== null);

    setPings((prev) => {
      const next: Record<string, PingState> = {};
      for (const t of targets) {
        next[t.key] = prev[t.key] ?? { status: "pending" };
      }
      return next;
    });

    let i = 0;
    const worker = async () => {
      while (!cancelled) {
        const idx = i++;
        if (idx >= targets.length) return;
        const { key, addr } = targets[idx];
        try {
          const result = await pingHost(addr.ip, addr.port, 2000);
          if (cancelled || pingSeqRef.current !== seq) return;
          setPings((prev) => ({ ...prev, [key]: { status: "done", result } }));
        } catch (e) {
          if (cancelled || pingSeqRef.current !== seq) return;
          setPings((prev) => ({
            ...prev,
            [key]: {
              status: "done",
              result: {
                ok: false,
                latency_ms: null,
                error: e instanceof Error ? e.message : String(e),
              },
            },
          }));
        }
      }
    };

    let cancelled = false;
    const workers = Array.from({ length: Math.min(8, targets.length) }, worker);
    Promise.all(workers);
    return () => {
      cancelled = true;
    };
  }, [servers]);

  const totalPlayers = servers?.reduce((s, sv) => s + sv.player_count, 0) ?? 0;
  const regions = servers
    ? [...new Set(servers.map((s) => s.region).filter(Boolean))]
    : [];

  // Sort: lower latency first (unknown/failed pings to the end),
  // tie-break by has-players, then by higher player count.
  const latencyOf = (sv: ServerListItem): number => {
    const p = pings[serverKey(sv)];
    if (p && p.status === "done" && p.result.ok && p.result.latency_ms != null) {
      return p.result.latency_ms;
    }
    if (typeof sv.ping === "number" && sv.ping > 0) return sv.ping;
    return Number.POSITIVE_INFINITY;
  };
  const sortedServers = servers
    ? [...servers].sort((a, b) => {
        const ha = a.player_count > 0 ? 1 : 0;
        const hb = b.player_count > 0 ? 1 : 0;
        if (ha !== hb) return hb - ha;
        const la = latencyOf(a);
        const lb = latencyOf(b);
        if (la !== lb) return la - lb;
        return b.player_count - a.player_count;
      })
    : null;

  return (
    <div className="p-6 space-y-5">
      {/* Stats bar */}
      <div className="flex gap-3">
        {[
          { label: "服务器", value: servers?.length ?? "—" },
          { label: "在线玩家", value: totalPlayers },
          { label: "地区", value: regions.length || "—" },
        ].map((s) => (
          <div
            key={s.label}
            className="flex-1 px-4 py-3 rounded-xl glass text-center"
          >
            <div className="text-2xl font-bold tabular-nums">{s.value}</div>
            <div className="text-[11px] text-white/50 mt-0.5">{s.label}</div>
          </div>
        ))}
      </div>

      {error && (
        <div className="text-sm text-red-300 px-3 py-2 rounded-lg bg-red-500/10">
          获取服务器列表失败：{error}
        </div>
      )}

      {servers === null && !error && (
        <div className="text-white/50 text-sm">加载中…</div>
      )}

      {servers && servers.length === 0 && (
        <div className="text-white/50 text-sm">暂无在线服务器</div>
      )}

      {sortedServers &&
        sortedServers.map((sv) => {
          const key = serverKey(sv);
          const addr = serverAddr(sv);
          const ping = pings[key];
          const players = sv.players ?? [];
          const hasPlayers = players.length > 0;
          const isExpanded = hasPlayers && expandedServers.has(key);
          const playerGroups = isExpanded ? groupPlayersByCountry(players) : [];
          const measuredLatency =
            ping?.status === "done" &&
            ping.result.ok &&
            ping.result.latency_ms != null
              ? ping.result.latency_ms
              : null;
          const reportedLatency =
            typeof sv.ping === "number" && sv.ping > 0 ? sv.ping : null;
          const displayLatency = measuredLatency ?? reportedLatency;

          const toggleExpanded = () => {
            if (!hasPlayers) return;
            setExpandedServers((current) => {
              const next = new Set(current);
              if (next.has(key)) next.delete(key);
              else next.add(key);
              return next;
            });
          };

          return (
            <GlassCard key={key} padding={false} className="overflow-hidden">
              <button
                type="button"
                onClick={toggleExpanded}
                disabled={!hasPlayers}
                aria-expanded={hasPlayers ? isExpanded : undefined}
                className={`w-full p-5 text-left flex items-start justify-between gap-4 transition-colors disabled:cursor-default ${
                  hasPlayers ? "hover:bg-white/[0.025]" : ""
                }`}
              >
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold truncate">
                    {sv.name}
                  </div>
                  <div className="flex items-center gap-3 mt-1.5 text-[11px] text-white/50 flex-wrap">
                    {sv.map && (
                      <img
                        src={mapImage(sv.map)}
                        alt={`地图：${mapName(sv.map)}`}
                        title={mapName(sv.map)}
                        loading="lazy"
                        className="h-10 w-20 rounded-md object-cover ring-1 ring-white/10"
                      />
                    )}
                    {sv.playlist && (
                      <span className="px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-300">
                        {gamemodeName(sv.playlist)}
                      </span>
                    )}
                    {sv.region && (
                      <span>{sv.region}</span>
                    )}
                    {sv.country && (
                      <span className="inline-flex items-center gap-1">
                        <img
                          src={flagImage(sv.country)}
                          alt=""
                          aria-hidden="true"
                          className="h-3 w-4 rounded-[2px] object-cover ring-1 ring-white/10"
                        />
                        {countryName(sv.country) || sv.country}
                      </span>
                    )}
                  </div>
                </div>
                <div className="shrink-0 flex items-center gap-3">
                  <div className="text-right flex flex-col items-end gap-1">
                    <div className="text-lg font-bold tabular-nums">
                      <span
                        className={
                          sv.player_count > 0
                            ? "text-emerald-300"
                            : "text-white/40"
                        }
                      >
                        {sv.player_count}
                      </span>
                      {sv.max_players != null && (
                        <span className="text-white/30 text-sm">
                          /{sv.max_players}
                        </span>
                      )}
                      <span className="text-[10px] text-white/40 ml-1">玩家</span>
                    </div>
                    <div className="text-[11px] tabular-nums">
                      {displayLatency != null ? (
                        <span className={pingColor(displayLatency)}>
                          {displayLatency} ms
                        </span>
                      ) : addr && (!ping || ping.status === "pending") ? (
                        <span className="text-white/40">测速中…</span>
                      ) : ping?.status === "done" ? (
                        <span
                          className="text-red-300/80"
                          title={ping.result.error ?? undefined}
                        >
                          超时
                        </span>
                      ) : (
                        <span className="text-white/30">延迟未上报</span>
                      )}
                    </div>
                  </div>
                  {hasPlayers && (
                    <span
                      aria-hidden="true"
                      className={`text-white/40 text-sm transition-transform ${
                        isExpanded ? "rotate-180" : ""
                      }`}
                    >
                      ▾
                    </span>
                  )}
                </div>
              </button>

              {isExpanded && (
                <div className="px-5 pb-5">
                  <div className="pt-4 border-t border-white/5 space-y-4">
                    {playerGroups.map((group) => (
                      <section key={group.key}>
                        <div className="flex items-center gap-2 mb-2 text-xs text-white/60">
                          <img
                            src={flagImage(group.code)}
                            alt=""
                            aria-hidden="true"
                            className="h-3 w-4 rounded-[2px] object-cover ring-1 ring-white/10"
                          />
                          <span className="font-medium text-white/75">
                            {group.label}
                          </span>
                          <span className="text-[10px] text-white/35 tabular-nums">
                            {group.players.length} 人
                          </span>
                        </div>
                        <div className="flex flex-wrap gap-1.5">
                          {group.players.map((player, index) => (
                            <span
                              key={`${player.name}-${index}`}
                              className="text-[11px] px-2 py-1 rounded-md bg-white/5 text-white/75 font-mono"
                            >
                              {player.name}
                            </span>
                          ))}
                        </div>
                      </section>
                    ))}
                  </div>
                </div>
              )}
            </GlassCard>
          );
        })}
    </div>
  );
}
