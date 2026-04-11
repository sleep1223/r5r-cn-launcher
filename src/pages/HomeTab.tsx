import { useEffect, useMemo, useState } from "react";
import clsx from "clsx";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { InstallProgress } from "../components/InstallProgress";
import { useSettings } from "../hooks/useSettings";
import { useLaunchExited } from "../hooks/useLaunchExited";
import { useInstallLog, useInstallProgress } from "../hooks/useInstallProgress";
import { useAccelerators } from "../hooks/useAccelerators";
import { autoAdoptExistingInstall, detectExistingR5R } from "../ipc/detect";
import { fetchRemoteConfig } from "../ipc/config";
import { fetchDashboardConfig } from "../ipc/dashboard";
import { openExternalUrl } from "../ipc/settings";
import { detectAccelerators } from "../ipc/accelerator";
import { getLauncherVersion, downloadAndApplyUpdate } from "../ipc/updater";
import { launchGame } from "../ipc/launch";
import {
  cancelInstall,
  checkUpdate,
  pauseInstall,
  startOfflineImport,
  startOnlineInstall,
  startRepair,
  startUpdate,
} from "../ipc/install";
import {
  DashboardConfig,
  DetectedInstall,
  LaunchOptionSelection,
  RemoteConfig,
} from "../ipc/types";
import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
import type { TabId } from "../components/Sidebar";

type Action = "install" | "update" | "play" | "blocked";

interface Props {
  onNavigate: (tab: TabId) => void;
}

export function HomeTab({ onNavigate }: Props) {
  const { settings, update, reload } = useSettings();
  const [detected, setDetected] = useState<DetectedInstall[] | null>(null);
  const [config, setConfig] = useState<RemoteConfig | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchedPid, setLaunchedPid] = useState<number | null>(null);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  // True for jobs that go through `run_install` (online install/update/repair).
  // Offline imports ignore pause — hide the button for them.
  const [activeJobPausable, setActiveJobPausable] = useState(false);
  const [jobPaused, setJobPaused] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<boolean | null>(null);
  const [remoteVersion, setRemoteVersion] = useState<string | null>(null);
  const [dashboard, setDashboard] = useState<DashboardConfig | null>(null);
  const [launcherUpdate, setLauncherUpdate] = useState<{
    version: string;
    url: string;
    force: boolean;
  } | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  // Flips true once the auto-adopt effect has finished (adopted or not).
  // We use this to suppress the onboarding card until we know whether the
  // backend will populate `library_root` from a detected official install.
  const [autoAdoptChecked, setAutoAdoptChecked] = useState(false);
  const exited = useLaunchExited();
  const progress = useInstallProgress();
  const installLogs = useInstallLog(activeJobId);
  const accelerators = useAccelerators();

  // Run detection once on mount.
  useEffect(() => {
    (async () => {
      try {
        const r = await detectExistingR5R();
        setDetected(r);
      } catch {
        setDetected([]);
      }
    })();
  }, []);

  // Auto-adopt: on first mount, check if the official R5Valkyrie launcher has
  // a LIVE install that we haven't adopted yet. If found, the backend writes
  // library_root + LIVE channel state into our settings — then we reload
  // settings and auto-trigger a verification so the user is ready to play.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await autoAdoptExistingInstall();
        if (cancelled) return;
        if (result.adopted) {
          // Settings were mutated by the backend — reload them into React
          // state so library_root / channels / selected_channel update.
          await reload();
          // Auto-trigger a verification (repair) to make sure all files
          // match the mirror's manifest.
          try {
            const id = await startRepair("LIVE");
            beginJob(id, true);
          } catch {
            // If repair fails to start (e.g. mirror URL not set), no big
            // deal — the user can still configure and run it manually.
          }
        }
      } catch {
        // Detection is best-effort — failure is silent.
      } finally {
        if (!cancelled) setAutoAdoptChecked(true);
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Pull the community dashboard once on mount. The URL is hardcoded in the
  // backend (`DEFAULT_DASHBOARD_API_URL`), so there's nothing to configure.
  // The dashboard is purely informational (announcement / rules / version
  // badge) and nothing in the install or launch flow depends on it, so we
  // silently degrade on failure rather than greeting users with a scary
  // error banner about a server-side outage they cannot fix. The Settings
  // tab's "测试一下" affordance still surfaces real errors to the maintainer.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const d = await fetchDashboardConfig();
        if (!cancelled) setDashboard(d);
      } catch (e) {
        if (!cancelled) {
          // eslint-disable-next-line no-console
          console.warn("dashboard fetch failed:", e);
          setDashboard(null);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Check for launcher self-update once the dashboard data arrives.
  useEffect(() => {
    if (!dashboard) return;
    if (!dashboard.launcher_version || !dashboard.launcher_update_url) return;
    let cancelled = false;
    (async () => {
      try {
        const { current } = await getLauncherVersion();
        if (cancelled) return;
        // Simple semver: split into [major, minor, patch] and compare.
        const parse = (s: string) =>
          s.replace(/^v/, "").split(".").map(Number);
        const c = parse(current);
        const r = parse(dashboard.launcher_version);
        const isNewer =
          r[0] > c[0] ||
          (r[0] === c[0] && r[1] > c[1]) ||
          (r[0] === c[0] && r[1] === c[1] && r[2] > c[2]);
        if (isNewer) {
          setLauncherUpdate({
            version: dashboard.launcher_version,
            url: dashboard.launcher_update_url,
            force: dashboard.force_update,
          });
        }
      } catch {
        // Version check failure is non-fatal — ignore.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [dashboard]);

  // If force_update is set and a valid update URL exists, auto-start the
  // update immediately — the user cannot skip.
  useEffect(() => {
    if (!launcherUpdate?.force || updating) return;
    if (!launcherUpdate.url) return;
    handleApplyUpdate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [launcherUpdate]);

  const handleApplyUpdate = async () => {
    if (!launcherUpdate?.url) return;
    setUpdating(true);
    setUpdateError(null);
    try {
      await downloadAndApplyUpdate(launcherUpdate.url);
      // If we get here the backend failed to exit (macOS). Show a message.
    } catch (e) {
      setUpdateError(e instanceof Error ? e.message : String(e));
      setUpdating(false);
    }
  };

  // Fetch the remote config whenever the URL changes.
  useEffect(() => {
    if (!settings?.root_config_url) {
      setConfig(null);
      setConfigError(null);
      return;
    }
    (async () => {
      setRefreshing(true);
      setConfigError(null);
      try {
        const c = await fetchRemoteConfig();
        setConfig(c);
        if (!settings.selected_channel && c.channels.length > 0) {
          const first = c.channels.find((ch) => ch.enabled) ?? c.channels[0];
          await update({ selected_channel: first.name });
        }
      } catch (e) {
        setConfigError(e instanceof Error ? e.message : String(e));
      } finally {
        setRefreshing(false);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings?.root_config_url]);

  // Check for updates when channel/install state changes.
  useEffect(() => {
    if (!settings?.selected_channel || !settings?.root_config_url) {
      setUpdateAvailable(null);
      return;
    }
    const installed = settings.channels[settings.selected_channel]?.installed;
    if (!installed) {
      setUpdateAvailable(null);
      return;
    }
    (async () => {
      try {
        const u = await checkUpdate(settings.selected_channel);
        setUpdateAvailable(u.has_update);
        setRemoteVersion(u.remote_version);
      } catch {
        setUpdateAvailable(null);
      }
    })();
  }, [settings?.selected_channel, settings?.channels, settings?.root_config_url]);

  // When an install completes, reload settings (so installed/version flips).
  useEffect(() => {
    if (
      progress?.job_id === activeJobId &&
      (progress.phase.phase === "complete" ||
        progress.phase.phase === "failed" ||
        progress.phase.phase === "cancelled")
    ) {
      void reload();
      setJobPaused(false);
      setActiveJobPausable(false);
      window.setTimeout(() => setActiveJobId(null), 1500);
    }
  }, [progress, activeJobId, reload]);

  // Helper: record a just-started job. `pausable` tells the UI whether the
  // 暂停 button should be offered — only `run_install` jobs honour it.
  const beginJob = (id: string, pausable: boolean) => {
    setActiveJobId(id);
    setActiveJobPausable(pausable);
    setJobPaused(false);
  };

  const installed =
    !!settings &&
    !!settings.selected_channel &&
    !!settings.channels[settings.selected_channel]?.installed;

  // Pick the detected install we'd launch from if the user hit "play" right
  // now. Only consider hits whose `path` directly contains `r5apex.exe` —
  // `has_game === true` — because `launch_game` joins `path/r5apex.exe` and
  // will error otherwise. Prefer a hit whose channel matches the selected
  // channel so, e.g., if the user chose LIVE we don't silently launch PTU.
  const launchableDetected = useMemo(() => {
    if (!detected || detected.length === 0) return null;
    const runnable = detected.filter((d) => d.has_game);
    if (runnable.length === 0) return null;
    const sel = settings?.selected_channel;
    if (sel) {
      const match = runnable.find(
        (d) => d.channel?.toUpperCase() === sel.toUpperCase(),
      );
      if (match) return match;
    }
    return runnable[0];
  }, [detected, settings?.selected_channel]);

  const action: Action = useMemo(() => {
    if (!settings) return "blocked";
    // If we have a detected install that can launch directly, always prefer
    // launching from it over going through install/update flows — the user
    // already has the game on disk, no need to re-download via the mirror.
    if (launchableDetected) return "play";
    if (!settings.root_config_url || !settings.library_root) {
      // Not configured for online install — but if a detected install exists,
      // user can still launch the game using compose options.
      if (detected && detected.length > 0) return "play";
      return "blocked";
    }
    if (!settings.selected_channel) return "blocked";
    if (!installed) return "install";
    if (updateAvailable) return "update";
    return "play";
  }, [settings, detected, launchableDetected, installed, updateAvailable]);

  const launchableDir = launchableDetected?.path ?? null;

  const handlePrimaryAction = async () => {
    if (!settings) return;
    setImportError(null);
    setLaunchError(null);
    setLaunchedPid(null);

    try {
      switch (action) {
        case "install": {
          const id = await startOnlineInstall(settings.selected_channel);
          beginJob(id, true);
          break;
        }
        case "update": {
          const id = await startUpdate(settings.selected_channel);
          beginJob(id, true);
          break;
        }
        case "play": {
          // Re-check accelerators *right before* launch — the cached hook
          // state might be 15s stale, and the user may have started a VPN
          // since then. Cheap (~10ms) so it's fine on the hot path.
          const fresh = await detectAccelerators().catch(() => []);
          if (fresh.length > 0) {
            // Include the actual process name so the user can track it down
            // in Task Manager if the friendly name is ambiguous (e.g. two
            // UU helpers running, or a "未知加速器" catch-all match).
            const names = fresh
              .map((a) => `${a.name}（${a.process_name}）`)
              .join("、");
            const ok = await ask(
              `检测到正在运行的加速器：${names}\n\n` +
                `社区服走的是镜像直连，加速器会把游戏流量绕到错误的节点，导致丢包、卡顿、断线。\n\n` +
                `建议先关闭加速器再启动游戏。是否仍要继续？`,
              {
                title: "检测到加速器",
                kind: "warning",
                okLabel: "仍要启动",
                cancelLabel: "取消",
              },
            );
            if (!ok) break;
          }
          const sel: LaunchOptionSelection =
            (settings.launch_option_selection as LaunchOptionSelection) ?? {
              items: {},
            };
          const channel = settings.selected_channel || "LIVE";
          const pid = await launchGame(channel, sel, launchableDir ?? undefined);
          setLaunchedPid(pid);
          break;
        }
        case "blocked":
          break;
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (action === "play") setLaunchError(msg);
      else setImportError(msg);
    }
  };

  const handleRepair = async () => {
    if (!settings?.selected_channel) return;
    setImportError(null);
    try {
      const id = await startRepair(settings.selected_channel);
      beginJob(id, true);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleImportDir = async () => {
    setImportError(null);
    if (!settings?.library_root) {
      setImportError("请先在【设置】中配置安装根目录");
      return;
    }
    const picked = await openDialog({
      directory: true,
      multiple: false,
      title: "选择离线包目录",
    });
    if (typeof picked !== "string") return;
    try {
      const id = await startOfflineImport(settings.library_root, {
        type: "directory",
        path: picked,
      });
      beginJob(id, false);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleImportZip = async () => {
    setImportError(null);
    if (!settings?.library_root) {
      setImportError("请先在【设置】中配置安装根目录");
      return;
    }
    const picked = await openDialog({
      directory: false,
      multiple: false,
      title: "选择离线包 zip",
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (typeof picked !== "string") return;
    try {
      const id = await startOfflineImport(settings.library_root, {
        type: "zip",
        path: picked,
      });
      beginJob(id, false);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCancelImport = async () => {
    if (!activeJobId) return;
    await cancelInstall(activeJobId);
  };

  const handleTogglePause = async () => {
    if (!activeJobId) return;
    const next = !jobPaused;
    // Flip local state immediately so the button updates even if the IPC
    // call is slow; if the backend rejects (job already finished), we'll
    // revert below.
    setJobPaused(next);
    try {
      const ok = await pauseInstall(activeJobId, next);
      if (!ok) setJobPaused(!next);
    } catch {
      setJobPaused(!next);
    }
  };

  const showingProgress = activeJobId && progress?.job_id === activeJobId;

  const actionLabel = (a: Action): string => {
    switch (a) {
      case "install":
        return "⬇ 安装游戏";
      case "update":
        return "↻ 更新游戏";
      case "play":
        return "▶ 启动游戏";
      case "blocked":
        return "请先在【设置】中配置代理与镜像";
    }
  };
  const actionVariant = (a: Action) => {
    switch (a) {
      case "install":
        return "primary" as const;
      case "update":
        return "warn" as const;
      case "play":
        return "success" as const;
      case "blocked":
        return "secondary" as const;
    }
  };

  // Force-update blocking overlay: covers the entire page so the user
  // cannot interact with anything until the update finishes.
  if (launcherUpdate?.force && updating) {
    return (
      <div className="p-6 flex items-center justify-center min-h-[60vh]">
        <GlassCard className="max-w-md text-center">
          <div className="text-xl font-semibold mb-3">正在更新启动器…</div>
          <div className="text-sm text-white/60 mb-4">
            新版本 {launcherUpdate.version} 正在下载并安装，完成后启动器将自动重启。
          </div>
          <div className="h-2 rounded-full bg-white/8 overflow-hidden">
            <div className="h-full bg-gradient-to-r from-blue-400 to-emerald-400 animate-pulse" />
          </div>
          {updateError && (
            <div className="text-xs text-red-300 mt-3">更新失败：{updateError}</div>
          )}
        </GlassCard>
      </div>
    );
  }

  // Show the onboarding card only once we know auto-adopt didn't fill in a
  // path for us — otherwise the card briefly flashes on cold start even
  // when the backend is about to adopt an existing official install.
  const needsGameFolder =
    !!settings && autoAdoptChecked && !settings.library_root;

  return (
    <div className="p-6 space-y-5">
      {needsGameFolder && (
        <GlassCard className="border-blue-400/40 bg-blue-500/[0.08]">
          <div className="flex items-start gap-4">
            <span className="text-2xl leading-none">📁</span>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-semibold text-blue-100">
                请先配置游戏文件夹
              </div>
              <div className="text-xs text-blue-100/75 mt-1 leading-relaxed">
                还没有选择游戏安装位置。前往【设置】选一个不含中文的目录作为
                R5R 库根目录，启动器会把游戏放在
                <span className="font-mono mx-1">
                  &lt;根目录&gt;/R5R Library/&lt;频道&gt;/
                </span>
                下。
              </div>
            </div>
            <PrimaryButton
              variant="primary"
              onClick={() => onNavigate("settings")}
            >
              前往设置
            </PrimaryButton>
          </div>
        </GlassCard>
      )}

      {/* Non-forced update banner — shown at the top, dismissible. */}
      {launcherUpdate && !launcherUpdate.force && (
        <GlassCard className="border-blue-400/30">
          <div className="flex items-center gap-4">
            <div className="flex-1 min-w-0">
              <div className="text-sm font-medium">
                启动器新版本 {launcherUpdate.version} 可用
              </div>
              <div className="text-xs text-white/50 mt-0.5">
                建议更新以获得最新功能和修复。
              </div>
              {updateError && (
                <div className="text-xs text-red-300 mt-1">
                  更新失败：{updateError}
                </div>
              )}
            </div>
            <PrimaryButton
              variant="primary"
              onClick={handleApplyUpdate}
              disabled={updating}
            >
              {updating ? "更新中…" : "立即更新"}
            </PrimaryButton>
            {!updating && (
              <PrimaryButton
                variant="secondary"
                onClick={() => setLauncherUpdate(null)}
              >
                稍后
              </PrimaryButton>
            )}
          </div>
        </GlassCard>
      )}

      <GlassCard className="relative overflow-hidden min-h-[340px]" padding={false}>
        <div className="absolute inset-0 bg-gradient-to-tr from-blue-500/10 via-transparent to-purple-500/10" />
        <div className="relative p-8 flex flex-col h-full">
          <div className="flex-1">
            <div className="text-3xl font-bold tracking-tight">
              R5R 中国镜像启动器
            </div>
            <div className="text-white/60 mt-2 max-w-xl">
              社区服专用 · 镜像加速 · 一键启动。
            </div>

            {config && (
              <div className="mt-6 space-y-3">
                {/* Channel picker — segmented pill row, one button per
                    channel. Each button shows the channel name and an
                    installed-state dot. Disabled channels are faded and
                    not clickable. */}
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-[10px] uppercase tracking-[0.18em] text-white/40 mr-1">
                    频道
                  </span>
                  {config.channels.map((c) => {
                    const isSelected = settings?.selected_channel === c.name;
                    const channelInstalled =
                      !!settings?.channels[c.name]?.installed;
                    return (
                      <button
                        key={c.name}
                        type="button"
                        disabled={!c.enabled}
                        onClick={() => update({ selected_channel: c.name })}
                        className={clsx(
                          "group relative px-3 py-1.5 rounded-lg text-xs font-medium transition-all border flex items-center gap-2",
                          !c.enabled &&
                            "opacity-40 cursor-not-allowed border-white/10 bg-white/[0.02] text-white/45",
                          c.enabled && isSelected &&
                            "border-blue-400/60 bg-blue-400/15 text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]",
                          c.enabled && !isSelected &&
                            "border-white/10 bg-white/[0.03] text-white/70 hover:bg-white/[0.06] hover:border-white/20",
                        )}
                      >
                        {/* Status dot — green = installed locally, hollow
                            ring = available, lock = disabled. */}
                        {!c.enabled ? (
                          <span className="text-[10px]">🔒</span>
                        ) : channelInstalled ? (
                          <span className="size-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.7)]" />
                        ) : (
                          <span className="size-1.5 rounded-full border border-white/40" />
                        )}
                        <span className="font-mono tracking-wide">
                          {c.name}
                        </span>
                      </button>
                    );
                  })}
                  {refreshing && (
                    <span className="text-xs text-white/40 ml-1">
                      <span className="inline-block size-1.5 rounded-full bg-blue-400 animate-pulse mr-1.5 align-middle" />
                      刷新中…
                    </span>
                  )}
                </div>

                {/* Version readout — local · remote · 有更新 / 已是最新. */}
                {(installed || remoteVersion || dashboard?.game_version) && (
                  <div className="flex items-center gap-3 flex-wrap text-[11px] font-mono tabular-nums">
                    {installed && (
                      <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-emerald-500/10 text-emerald-300 border border-emerald-400/20">
                        <span className="text-white/45">本地</span>
                        {settings.channels[settings.selected_channel]?.version ||
                          "—"}
                      </span>
                    )}
                    {remoteVersion && (
                      <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-white/[0.04] text-white/65 border border-white/10">
                        <span className="text-white/40">远端</span>
                        {remoteVersion}
                      </span>
                    )}
                    {dashboard?.game_version && (
                      <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-blue-500/10 text-blue-300 border border-blue-400/20">
                        <span className="text-white/45">社区服</span>
                        {dashboard.game_version}
                      </span>
                    )}
                    {installed && updateAvailable && (
                      <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-amber-500/15 text-amber-300 border border-amber-400/30">
                        ↻ 有更新
                      </span>
                    )}
                    {installed && updateAvailable === false && (
                      <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-emerald-500/10 text-emerald-300/80 border border-emerald-400/20">
                        ✓ 已是最新
                      </span>
                    )}
                  </div>
                )}
              </div>
            )}

            {configError && (
              <div className="mt-4 text-xs px-3 py-2 rounded-lg bg-red-500/10 text-red-300 max-w-xl">
                获取镜像 config 失败：{configError}
              </div>
            )}
          </div>

          {showingProgress ? (
            <div className="mt-6">
              <InstallProgress
                progress={progress!}
                logs={installLogs}
                onCancel={handleCancelImport}
                onTogglePause={activeJobPausable ? handleTogglePause : undefined}
                paused={jobPaused}
              />
            </div>
          ) : (
            <div className="space-y-3 mt-6">
              <div className="flex items-center gap-3 flex-wrap">
                <PrimaryButton
                  variant={actionVariant(action)}
                  size="lg"
                  disabled={action === "blocked"}
                  onClick={handlePrimaryAction}
                >
                  {actionLabel(action)}
                </PrimaryButton>
                <PrimaryButton variant="secondary" onClick={handleImportDir}>
                  导入离线包目录
                </PrimaryButton>
                <PrimaryButton variant="secondary" onClick={handleImportZip}>
                  导入离线包 zip
                </PrimaryButton>
                <PrimaryButton
                  variant="secondary"
                  onClick={handleRepair}
                  disabled={!settings?.selected_channel || !settings?.library_root}
                >
                  校验
                </PrimaryButton>
              </div>
              {launchableDir && action === "play" && (
                <div className="text-xs text-white/40">
                  将从已检测到的官方安装启动：{launchableDir}
                </div>
              )}
              {launchedPid !== null && (
                <div className="text-xs text-emerald-300">
                  已启动 (PID {launchedPid})
                  {exited && exited.pid === launchedPid && (
                    <> · 游戏已退出 (code {exited.code ?? "?"})</>
                  )}
                </div>
              )}
              {launchError && (
                <div className="text-xs text-red-300">启动失败：{launchError}</div>
              )}
              {importError && (
                <div className="text-xs text-red-300">操作失败：{importError}</div>
              )}
            </div>
          )}
        </div>
      </GlassCard>

      {accelerators.length > 0 && (
        <GlassCard className="border-amber-400/30 bg-amber-500/[0.06]">
          <div className="flex items-start gap-3">
            <span className="text-2xl leading-none">⚠</span>
            <div className="flex-1 min-w-0">
              <div className="text-sm font-semibold text-amber-200">
                检测到加速器正在运行
              </div>
              <div className="text-xs text-amber-100/80 mt-1 leading-relaxed">
                社区服走镜像直连，开启加速器会把游戏流量绕到错误的节点，
                <span className="font-semibold">导致丢包、卡顿、甚至断线</span>
                。建议在启动游戏前先关闭加速器。
              </div>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {accelerators.map((a) => (
                  <span
                    key={`${a.name}-${a.pid}`}
                    className="text-[11px] px-2 py-0.5 rounded bg-amber-500/15 text-amber-200 font-mono"
                  >
                    {a.name}
                    <span className="text-amber-300/50 ml-1">
                      ({a.process_name})
                    </span>
                  </span>
                ))}
              </div>
            </div>
          </div>
        </GlassCard>
      )}

      {dashboard?.announcement &&
        (dashboard.announcement.title || dashboard.announcement.content) && (
          <GlassCard>
            <SectionHeader
              icon="📣"
              title={dashboard.announcement.title || "公告"}
            />
            <div className="text-sm text-white/75 whitespace-pre-wrap leading-relaxed">
              {dashboard.announcement.content}
            </div>
            <div className="mt-4 flex flex-wrap gap-2">
              {dashboard.docs_url && (
                <PrimaryButton
                  variant="secondary"
                  onClick={() => openExternalUrl(dashboard.docs_url)}
                >
                  📖 查看文档
                </PrimaryButton>
              )}
              {dashboard.offline_package_url && (
                <PrimaryButton
                  variant="secondary"
                  onClick={() => openExternalUrl(dashboard.offline_package_url)}
                >
                  📦 离线包下载
                </PrimaryButton>
              )}
            </div>
          </GlassCard>
        )}

      {dashboard && dashboard.rules.length > 0 && (
        <GlassCard>
          <SectionHeader icon="📜" title="服务器规则" />
          <ul className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {dashboard.rules.map((r, i) => (
              <li
                key={`${r.text}-${i}`}
                className="text-sm bg-white/5 rounded-lg px-3 py-2 flex items-center gap-2"
              >
                <span className="text-base leading-none">{r.icon}</span>
                <span className="text-white/80">{r.text}</span>
              </li>
            ))}
          </ul>
        </GlassCard>
      )}

      <GlassCard>
        <SectionHeader
          icon="🔎"
          title="已检测到的官方 R5Reloaded 安装"
          subtitle="便于你参考路径避免重装；社区服需要安装到一个新的、不含中文的目录。"
        />
        {detected === null && (
          <div className="text-sm text-white/40">检测中…</div>
        )}
        {detected && detected.length === 0 && (
          <div className="text-sm text-white/50">
            未检测到官方 R5Reloaded 安装。
            {navigator.userAgent.includes("Mac") &&
              "（macOS 上不支持检测，请在 Windows 上使用。）"}
          </div>
        )}
        {detected && detected.length > 0 && (
          <ul className="space-y-2">
            {detected.map((d, i) => (
              <li
                key={`${d.source}-${i}`}
                className="text-sm bg-white/5 rounded-lg px-3 py-2"
              >
                <div className="font-mono text-xs truncate">{d.path}</div>
                <div className="text-[11px] text-white/40 mt-0.5">
                  来源：{sourceLabel(d.source)}
                  {d.version && ` · 版本 ${d.version}`}
                  {d.channel && ` · 频道 ${d.channel}`}
                </div>
              </li>
            ))}
          </ul>
        )}
      </GlassCard>
    </div>
  );
}

function sourceLabel(s: DetectedInstall["source"]): string {
  switch (s) {
    case "shortcut":
      return "开始菜单快捷方式";
    case "registry":
      return "卸载注册表";
    case "library_scan":
      return "目录扫描";
  }
}
