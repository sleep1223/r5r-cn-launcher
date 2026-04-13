import { useEffect, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { useAuth } from "../hooks/useAuth";
import {
  getPlayerVsAll,
  getPlayerWeapons,
  PlayerVsAllRecord,
  PlayerVsAllSummary,
  WeaponRecord,
} from "../api";
import { weaponName } from "../utils/maps";

export function ProfileTab() {
  const { user } = useAuth();
  const [vsAll, setVsAll] = useState<PlayerVsAllRecord[] | null>(null);
  const [summary, setSummary] = useState<PlayerVsAllSummary | null>(null);
  const [weapons, setWeapons] = useState<WeaponRecord[] | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!user) return;
    let cancelled = false;
    setLoading(true);
    (async () => {
      try {
        const [vs, wp] = await Promise.all([
          getPlayerVsAll(user.player_name, { page_size: 50, sort: "kills" }),
          getPlayerWeapons(user.player_name, { page_size: 50, sort: "kills" }),
        ]);
        if (!cancelled) {
          setVsAll(vs.data);
          setSummary(vs.summary ?? null);
          setWeapons(wp);
        }
      } catch { /* silent */ }
      finally { if (!cancelled) setLoading(false); }
    })();
    return () => { cancelled = true; };
  }, [user]);

  if (!user) {
    return (
      <div className="p-6">
        <GlassCard>
          <div className="text-center text-white/50 py-8">
            请先在「组队」页面登录后查看个人数据。
          </div>
        </GlassCard>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-5">
      {/* Player header */}
      <GlassCard>
        <div className="flex items-center gap-4">
          <div className="size-12 rounded-full bg-blue-400/20 flex items-center justify-center text-xl font-bold text-blue-300">
            {user.player_name[0]?.toUpperCase()}
          </div>
          <div>
            <div className="text-lg font-semibold">{user.player_name}</div>
            {summary && (
              <div className="flex gap-3 mt-1 text-[11px]">
                <span className="text-emerald-300">
                  击杀 {summary.total_kills}
                </span>
                <span className="text-red-300/80">
                  死亡 {summary.total_deaths}
                </span>
                <span className="font-medium">
                  K/D {summary.kd.toFixed(2)}
                </span>
              </div>
            )}
          </div>
        </div>
        {summary?.nemesis && (
          <div className="mt-3 text-xs text-white/50">
            宿敌：
            <span className="text-red-300 font-mono ml-1">
              {summary.nemesis.opponent_name}
            </span>
            <span className="ml-2">
              (被击杀 {summary.nemesis.deaths} 次)
            </span>
          </div>
        )}
      </GlassCard>

      {loading && <div className="text-white/50 text-sm">加载中…</div>}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-5">
        {/* Kill stats vs opponents */}
        <GlassCard>
          <SectionHeader title="对手击杀统计" />
          <div className="overflow-y-auto max-h-[400px]">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-[#1b2026]">
                <tr className="text-white/40 text-[11px] border-b border-white/5">
                  <th className="text-left py-2 px-2">对手</th>
                  <th className="text-right py-2 px-2">击杀</th>
                  <th className="text-right py-2 px-2">死亡</th>
                  <th className="text-right py-2 px-2">K/D</th>
                </tr>
              </thead>
              <tbody>
                {vsAll?.map((r) => (
                  <tr
                    key={r.opponent_name}
                    className="border-b border-white/[0.03] hover:bg-white/[0.03]"
                  >
                    <td className="py-1.5 px-2 font-mono text-xs text-white/90">
                      {r.opponent_name}
                    </td>
                    <td className="py-1.5 px-2 text-right text-emerald-300 tabular-nums text-xs">
                      {r.kills}
                    </td>
                    <td className="py-1.5 px-2 text-right text-red-300/80 tabular-nums text-xs">
                      {r.deaths}
                    </td>
                    <td className="py-1.5 px-2 text-right tabular-nums text-xs font-medium">
                      {r.kd.toFixed(2)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {vsAll && vsAll.length === 0 && (
              <div className="text-center text-white/40 py-6 text-sm">
                暂无数据
              </div>
            )}
          </div>
        </GlassCard>

        {/* Weapon stats */}
        <GlassCard>
          <SectionHeader title="武器统计" />
          <div className="overflow-y-auto max-h-[400px]">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-[#1b2026]">
                <tr className="text-white/40 text-[11px] border-b border-white/5">
                  <th className="text-left py-2 px-2">武器</th>
                  <th className="text-right py-2 px-2">击杀</th>
                  <th className="text-right py-2 px-2">死亡</th>
                  <th className="text-right py-2 px-2">K/D</th>
                </tr>
              </thead>
              <tbody>
                {weapons?.map((r) => (
                  <tr
                    key={r.weapon}
                    className="border-b border-white/[0.03] hover:bg-white/[0.03]"
                  >
                    <td className="py-1.5 px-2 text-xs text-white/80">
                      {weaponName(r.weapon)}
                    </td>
                    <td className="py-1.5 px-2 text-right text-emerald-300 tabular-nums text-xs">
                      {r.kills}
                    </td>
                    <td className="py-1.5 px-2 text-right text-red-300/80 tabular-nums text-xs">
                      {r.deaths}
                    </td>
                    <td className="py-1.5 px-2 text-right tabular-nums text-xs font-medium">
                      {r.kd.toFixed(2)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {weapons && weapons.length === 0 && (
              <div className="text-center text-white/40 py-6 text-sm">
                暂无数据
              </div>
            )}
          </div>
        </GlassCard>
      </div>
    </div>
  );
}
