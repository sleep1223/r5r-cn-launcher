import { useEffect, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { useAuth } from "../hooks/useAuth";
import {
  getPlayerVsAll,
  getPlayerWeapons,
  PlayerVsAllRecord,
  PlayerVsAllSummary,
  StatsTimeRange,
  WeaponRecord,
} from "../api";
import { InputDeviceBadge } from "../components/InputDeviceBadge";
import { weaponName } from "../utils/maps";

const RANGES: { value: StatsTimeRange; label: string }[] = [
  { value: "today", label: "今日" },
  { value: "yesterday", label: "昨日" },
  { value: "week", label: "本周" },
  { value: "last_week", label: "上周" },
  { value: "month", label: "本月" },
  { value: "all", label: "全部" },
];

export function ProfileTab() {
  const { user } = useAuth();
  const [vsAll, setVsAll] = useState<PlayerVsAllRecord[] | null>(null);
  const [summary, setSummary] = useState<PlayerVsAllSummary | null>(null);
  const [weapons, setWeapons] = useState<WeaponRecord[] | null>(null);
  const [range, setRange] = useState<StatsTimeRange>("all");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!user) return;
    let cancelled = false;
    setLoading(true);
    setVsAll(null);
    setSummary(null);
    setWeapons(null);
    (async () => {
      try {
        const [vs, wp] = await Promise.all([
          getPlayerVsAll(user.player_name, {
            page_size: 50,
            sort: "kills",
            range,
          }),
          getPlayerWeapons(user.player_name, {
            page_size: 50,
            sort: "kills",
            range,
          }),
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
  }, [range, user]);

  if (!user) {
    return (
      <div className="p-6">
        <GlassCard>
          <div className="text-center text-white/50 py-8">
            请先点击左下角的「登录」并输入 AppKey，再查看个人数据。
          </div>
        </GlassCard>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-5">
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div>
          <div className="text-sm font-medium">统计时间</div>
          <div className="text-[11px] text-white/40 mt-0.5">
            对手和武器统计使用相同时间范围
          </div>
        </div>
        <div
          role="group"
          aria-label="统计时间范围"
          className="flex gap-1 flex-wrap"
        >
          {RANGES.map((item) => (
            <button
              key={item.value}
              type="button"
              onClick={() => setRange(item.value)}
              aria-pressed={range === item.value}
              className={`px-2.5 py-1 rounded-md text-[11px] border transition-all ${
                range === item.value
                  ? "border-blue-400/50 bg-blue-400/10 text-white"
                  : "border-white/10 text-white/50 hover:bg-white/5"
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>

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
            <span className="ml-2 inline-flex align-middle">
              <InputDeviceBadge
                device={summary.nemesis.input_device}
                compact
              />
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
          <div className="overflow-auto max-h-[400px]">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-[#1b2026]">
                <tr className="text-white/40 text-[11px] border-b border-white/5">
                  <th className="text-left py-2 px-2">对手</th>
                  <th className="text-left py-2 px-2">输入设备</th>
                  <th className="text-right py-2 px-2">击杀</th>
                  <th className="text-right py-2 px-2">死亡</th>
                  <th className="text-right py-2 px-2">K/D</th>
                </tr>
              </thead>
              <tbody>
                {vsAll?.map((r) => (
                  <tr
                    key={`${r.opponent_id ?? r.opponent_name}-${r.input_device ?? "unknown"}`}
                    className="border-b border-white/[0.03] hover:bg-white/[0.03]"
                  >
                    <td className="py-1.5 px-2 font-mono text-xs text-white/90">
                      {r.opponent_name}
                    </td>
                    <td className="py-1.5 px-2">
                      <InputDeviceBadge device={r.input_device} compact />
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
                {weapons?.map((r, index) => (
                  <tr
                    key={`${r.weapon}-${r.input_device ?? "unknown"}-${index}`}
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
