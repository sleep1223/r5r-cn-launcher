import { useEffect, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { useAuth } from "../hooks/useAuth";
import {
  getTeams,
  createTeam,
  joinTeam,
  cancelTeam,
  leaveTeam,
  Team,
} from "../api";

export function TeamsTab() {
  const { appKey, user, loading: authLoading, error: authError, login, logout } = useAuth();
  const [keyInput, setKeyInput] = useState("");
  const [teams, setTeams] = useState<Team[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [opError, setOpError] = useState<string | null>(null);

  const isLoggedIn = !!appKey && !!user;

  // Poll teams every 5s.
  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const r = await getTeams({ page_size: 50 });
        if (!cancelled) setTeams(r.data);
      } catch { /* silent */ }
    };
    load();
    const t = setInterval(load, 5000);
    return () => { cancelled = true; clearInterval(t); };
  }, []);

  const handleLogin = async () => {
    if (!keyInput.trim()) return;
    await login(keyInput.trim());
  };

  const handleCreate = async (slots: number) => {
    if (!appKey) return;
    setBusy(true);
    setOpError(null);
    try {
      await createTeam(appKey, slots);
    } catch (e) {
      setOpError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleJoin = async (teamId: number) => {
    if (!appKey) return;
    setBusy(true);
    setOpError(null);
    try {
      await joinTeam(appKey, teamId);
    } catch (e) {
      setOpError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleCancel = async (teamId: number) => {
    if (!appKey) return;
    setBusy(true);
    try {
      await cancelTeam(appKey, teamId);
    } catch { /* silent */ }
    finally { setBusy(false); }
  };

  const handleLeave = async (teamId: number) => {
    if (!appKey) return;
    setBusy(true);
    try {
      await leaveTeam(appKey, teamId);
    } catch { /* silent */ }
    finally { setBusy(false); }
  };

  const statusLabel = (s: Team["status"]) => {
    switch (s) {
      case "open": return "招募中";
      case "full": return "已满";
      case "cancelled": return "已取消";
      case "expired": return "已过期";
    }
  };

  const statusColor = (s: Team["status"]) => {
    switch (s) {
      case "open": return "bg-emerald-500/15 text-emerald-300 border-emerald-400/30";
      case "full": return "bg-blue-500/15 text-blue-300 border-blue-400/30";
      default: return "bg-white/5 text-white/40 border-white/10";
    }
  };

  const isMember = (team: Team) =>
    !!user && team.members.some((m) => m.player_id === user.player_id);
  const isCreator = (team: Team) =>
    !!user && team.creator.player_id === user.player_id;

  return (
    <div className="p-6 space-y-5">
      {/* Auth section */}
      {!isLoggedIn ? (
        <GlassCard>
          <SectionHeader title="登录" subtitle="输入你的 AppKey 以使用组队功能。" />
          <div className="flex gap-2">
            <input
              type="password"
              placeholder="AppKey"
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleLogin()}
            />
            <PrimaryButton
              variant="primary"
              onClick={handleLogin}
              disabled={authLoading}
            >
              {authLoading ? "登录中…" : "登录"}
            </PrimaryButton>
          </div>
          {authError && (
            <div className="text-xs text-red-300 mt-2">{authError}</div>
          )}
        </GlassCard>
      ) : (
        <GlassCard>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="size-8 rounded-full bg-blue-400/20 flex items-center justify-center text-sm font-bold text-blue-300">
                {user.player_name[0]?.toUpperCase()}
              </div>
              <div>
                <div className="text-sm font-medium">{user.player_name}</div>
                <div className="text-[11px] text-white/40">已登录</div>
              </div>
            </div>
            <div className="flex gap-2">
              <PrimaryButton
                variant="secondary"
                onClick={() => handleCreate(2)}
                disabled={busy}
              >
                创建队伍 (3人)
              </PrimaryButton>
              <PrimaryButton variant="secondary" onClick={logout}>
                登出
              </PrimaryButton>
            </div>
          </div>
        </GlassCard>
      )}

      {opError && (
        <div className="text-xs text-red-300 px-3 py-2 rounded-lg bg-red-500/10">
          {opError}
        </div>
      )}

      {/* Teams list */}
      <SectionHeader title="组队大厅" subtitle="每 5 秒自动刷新" />
      {teams === null && (
        <div className="text-white/50 text-sm">加载中…</div>
      )}
      {teams && teams.length === 0 && (
        <div className="text-white/50 text-sm">暂无队伍，登录后可创建</div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {teams?.map((team) => (
          <GlassCard key={team.id}>
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2">
                <span className="text-xs font-mono text-white/40">
                  #{team.id}
                </span>
                <span
                  className={`text-[10px] px-1.5 py-0.5 rounded border ${statusColor(team.status)}`}
                >
                  {statusLabel(team.status)}
                </span>
              </div>
              <span className="text-[10px] text-white/30">
                {new Date(team.created_at).toLocaleTimeString("zh-CN")}
              </span>
            </div>

            {/* Members */}
            <div className="space-y-1.5">
              {team.members.map((m) => (
                <div
                  key={m.player_id}
                  className="flex items-center justify-between px-2 py-1.5 rounded-md bg-white/[0.03]"
                >
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-xs text-white/90">
                      {m.player_name}
                    </span>
                    {m.role === "creator" && (
                      <span className="text-[9px] px-1 py-0.5 rounded bg-amber-500/15 text-amber-300">
                        队长
                      </span>
                    )}
                  </div>
                  <span className="text-[11px] text-white/50 tabular-nums">
                    K/D {m.kd.toFixed(2)}
                  </span>
                </div>
              ))}
              {/* Empty slots */}
              {Array.from({ length: team.slots_remaining }).map((_, i) => (
                <div
                  key={`empty-${i}`}
                  className="px-2 py-1.5 rounded-md border border-dashed border-white/10 text-center text-[11px] text-white/25"
                >
                  空位
                </div>
              ))}
            </div>

            {/* Actions */}
            {isLoggedIn && team.status === "open" && (
              <div className="flex gap-2 mt-3">
                {isCreator(team) && (
                  <PrimaryButton
                    variant="secondary"
                    onClick={() => handleCancel(team.id)}
                    disabled={busy}
                  >
                    取消队伍
                  </PrimaryButton>
                )}
                {isMember(team) && !isCreator(team) && (
                  <PrimaryButton
                    variant="secondary"
                    onClick={() => handleLeave(team.id)}
                    disabled={busy}
                  >
                    离开
                  </PrimaryButton>
                )}
                {!isMember(team) && team.slots_remaining > 0 && (
                  <PrimaryButton
                    variant="primary"
                    onClick={() => handleJoin(team.id)}
                    disabled={busy}
                  >
                    加入
                  </PrimaryButton>
                )}
              </div>
            )}
          </GlassCard>
        ))}
      </div>
    </div>
  );
}
