import { useEffect, useState } from "react";
import { GlassCard } from "../components/GlassCard";
import { getServers, ServerListItem } from "../api";
import { gamemodeName, mapName, countryName } from "../utils/maps";

export function ServersTab() {
  const [servers, setServers] = useState<ServerListItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  const totalPlayers = servers?.reduce((s, sv) => s + sv.player_count, 0) ?? 0;
  const regions = servers
    ? [...new Set(servers.map((s) => s.region).filter(Boolean))]
    : [];

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

      {servers &&
        servers.map((sv) => (
          <GlassCard key={`${sv.name}-${sv.host}-${sv.port}`}>
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
                </div>
              </div>
              <div className="text-right shrink-0">
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
                </div>
                <div className="text-[10px] text-white/40">玩家</div>
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
        ))}
    </div>
  );
}
