import { useEffect, useRef, useState } from "react";
import { GlassCard } from "../components/GlassCard";
import { getServers, ServerListItem } from "../api";
import { gamemodeName, mapName, countryName } from "../utils/maps";
import { pingHost, PingResult } from "../ipc/ping";

type PingState = { status: "pending" } | { status: "done"; result: PingResult };

const serverKey = (sv: ServerListItem) => `${sv.name}-${sv.ip}-${sv.port}`;
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

export function ServersTab() {
  const [servers, setServers] = useState<ServerListItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pings, setPings] = useState<Record<string, PingState>>({});
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
  // tie-break by higher player count.
  const latencyOf = (sv: ServerListItem): number => {
    const p = pings[serverKey(sv)];
    if (p && p.status === "done" && p.result.ok && p.result.latency_ms != null) {
      return p.result.latency_ms;
    }
    return Number.POSITIVE_INFINITY;
  };
  const sortedServers = servers
    ? [...servers].sort((a, b) => {
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
          return (
            <GlassCard key={key}>
              <div className="flex items-start justify-between gap-4">
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold truncate">
                    {sv.name}
                  </div>
                  <div className="flex items-center gap-3 mt-1.5 text-[11px] text-white/50 flex-wrap">
                    {sv.map && (
                      <span className="px-1.5 py-0.5 rounded bg-white/5">
                        {mapName(sv.map)}
                      </span>
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
                      <span>{countryName(sv.country)}</span>
                    )}
                    {addr && (
                      <span className="font-mono text-white/40">
                        {addr.ip}:{addr.port}
                      </span>
                    )}
                  </div>
                </div>
                <div className="text-right shrink-0 flex flex-col items-end gap-1">
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
                    {!addr ? (
                      <span className="text-white/30">无地址</span>
                    ) : !ping || ping.status === "pending" ? (
                      <span className="text-white/40">测速中…</span>
                    ) : ping.result.ok && ping.result.latency_ms != null ? (
                      <span className={pingColor(ping.result.latency_ms)}>
                        {ping.result.latency_ms} ms
                      </span>
                    ) : (
                      <span
                        className="text-red-300/80"
                        title={ping.result.error ?? undefined}
                      >
                        超时
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Player list (expandable) */}
              {sv.players && sv.players.length > 0 && (
                <div className="mt-3 pt-3 border-t border-white/5">
                  <div className="flex flex-wrap gap-1.5">
                    {sv.players.map((p) => (
                      <span
                        key={p.name}
                        className="text-[11px] px-2 py-0.5 rounded bg-white/5 text-white/70 font-mono"
                      >
                        {p.name}
                      </span>
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
