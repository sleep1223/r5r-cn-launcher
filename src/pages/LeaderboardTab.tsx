import { useEffect, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import {
  getKillLeaderboard,
  getWeaponLeaderboard,
  KdRecord,
  WeaponLeaderboardRecord,
} from "../api";

type TimeRange = "today" | "yesterday" | "week" | "month" | "all";
type Board = "kills" | "weapons";

const RANGES: { value: TimeRange; label: string }[] = [
  { value: "today", label: "今日" },
  { value: "yesterday", label: "昨日" },
  { value: "week", label: "本周" },
  { value: "month", label: "本月" },
  { value: "all", label: "全部" },
];

export function LeaderboardTab() {
  const [board, setBoard] = useState<Board>("kills");
  const [range, setRange] = useState<TimeRange>("week");
  const [killData, setKillData] = useState<KdRecord[] | null>(null);
  const [weaponData, setWeaponData] = useState<WeaponLeaderboardRecord[] | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        if (board === "kills") {
          const d = await getKillLeaderboard({
            range,
            page_size: 50,
            sort: "kills",
          });
          if (!cancelled) setKillData(d);
        } else {
          const d = await getWeaponLeaderboard({
            range,
            page_size: 50,
            sort: "kills",
          });
          if (!cancelled) setWeaponData(d);
        }
      } catch {
        /* silent */
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [board, range]);

  return (
    <div className="p-6 space-y-4">
      {/* Board + Range selector */}
      <div className="flex items-center gap-3 flex-wrap">
        <div className="flex gap-1">
          {(["kills", "weapons"] as const).map((b) => (
            <button
              key={b}
              onClick={() => setBoard(b)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium border transition-all ${
                board === b
                  ? "border-blue-400/60 bg-blue-400/10 text-white"
                  : "border-white/10 text-white/60 hover:bg-white/5"
              }`}
            >
              {b === "kills" ? "击杀排行" : "武器排行"}
            </button>
          ))}
        </div>
        <div className="w-px h-5 bg-white/10" />
        <div className="flex gap-1">
          {RANGES.map((r) => (
            <button
              key={r.value}
              onClick={() => setRange(r.value)}
              className={`px-2.5 py-1 rounded-md text-[11px] border transition-all ${
                range === r.value
                  ? "border-blue-400/50 bg-blue-400/10 text-white"
                  : "border-white/10 text-white/50 hover:bg-white/5"
              }`}
            >
              {r.label}
            </button>
          ))}
        </div>
      </div>

      {loading && <div className="text-white/50 text-sm">加载中…</div>}

      {/* Kill leaderboard */}
      {board === "kills" && killData && (
        <GlassCard>
          <SectionHeader title="击杀排行榜" />
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-white/40 text-[11px] border-b border-white/5">
                  <th className="text-left py-2 px-2 w-12">#</th>
                  <th className="text-left py-2 px-2">玩家</th>
                  <th className="text-right py-2 px-2">击杀</th>
                  <th className="text-right py-2 px-2">死亡</th>
                  <th className="text-right py-2 px-2">K/D</th>
                </tr>
              </thead>
              <tbody>
                {killData.map((r, i) => (
                  <tr
                    key={r.name}
                    className="border-b border-white/[0.03] hover:bg-white/[0.03]"
                  >
                    <td className="py-2 px-2">
                      <RankBadge rank={i + 1} />
                    </td>
                    <td className="py-2 px-2 font-mono text-white/90">
                      {r.name}
                    </td>
                    <td className="py-2 px-2 text-right text-emerald-300 tabular-nums">
                      {r.kills}
                    </td>
                    <td className="py-2 px-2 text-right text-red-300/80 tabular-nums">
                      {r.deaths}
                    </td>
                    <td className="py-2 px-2 text-right tabular-nums font-medium">
                      {r.kd.toFixed(2)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {killData.length === 0 && (
            <div className="text-center text-white/40 py-6 text-sm">
              暂无数据
            </div>
          )}
        </GlassCard>
      )}

      {/* Weapon leaderboard */}
      {board === "weapons" && weaponData && (
        <GlassCard>
          <SectionHeader title="武器排行榜" />
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-white/40 text-[11px] border-b border-white/5">
                  <th className="text-left py-2 px-2 w-12">#</th>
                  <th className="text-left py-2 px-2">武器</th>
                  <th className="text-left py-2 px-2">玩家</th>
                  <th className="text-right py-2 px-2">击杀</th>
                  <th className="text-right py-2 px-2">死亡</th>
                  <th className="text-right py-2 px-2">K/D</th>
                </tr>
              </thead>
              <tbody>
                {weaponData.map((r, i) => (
                  <tr
                    key={`${r.weapon}-${r.name}`}
                    className="border-b border-white/[0.03] hover:bg-white/[0.03]"
                  >
                    <td className="py-2 px-2">
                      <RankBadge rank={i + 1} />
                    </td>
                    <td className="py-2 px-2 text-white/70">{r.weapon}</td>
                    <td className="py-2 px-2 font-mono text-white/90">
                      {r.name}
                    </td>
                    <td className="py-2 px-2 text-right text-emerald-300 tabular-nums">
                      {r.kills}
                    </td>
                    <td className="py-2 px-2 text-right text-red-300/80 tabular-nums">
                      {r.deaths}
                    </td>
                    <td className="py-2 px-2 text-right tabular-nums font-medium">
                      {r.kd.toFixed(2)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {weaponData.length === 0 && (
            <div className="text-center text-white/40 py-6 text-sm">
              暂无数据
            </div>
          )}
        </GlassCard>
      )}
    </div>
  );
}

function RankBadge({ rank }: { rank: number }) {
  if (rank === 1)
    return (
      <span className="inline-flex size-6 items-center justify-center rounded-full bg-amber-400/20 text-amber-300 text-[11px] font-bold">
        1
      </span>
    );
  if (rank === 2)
    return (
      <span className="inline-flex size-6 items-center justify-center rounded-full bg-gray-300/15 text-gray-300 text-[11px] font-bold">
        2
      </span>
    );
  if (rank === 3)
    return (
      <span className="inline-flex size-6 items-center justify-center rounded-full bg-orange-400/15 text-orange-300 text-[11px] font-bold">
        3
      </span>
    );
  return <span className="text-white/40 text-[11px] pl-1">{rank}</span>;
}
