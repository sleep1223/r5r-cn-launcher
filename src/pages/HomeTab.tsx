import { useEffect, useMemo, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { InstallProgress } from "../components/InstallProgress";
import { useSettings } from "../hooks/useSettings";
import { useLaunchExited } from "../hooks/useLaunchExited";
import { useInstallLog, useInstallProgress } from "../hooks/useInstallProgress";
import { useAccelerators } from "../hooks/useAccelerators";
import { autoAdoptExistingInstall, detectExistingR5R } from "../ipc/detect";
import { fetchDashboardConfig } from "../ipc/dashboard";
import { openExternalUrl, suggestInstallPath } from "../ipc/settings";
import { detectAccelerators } from "../ipc/accelerator";
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
  DiskSuggestion,
  LaunchOptionSelection,
  ProgressEvent,
} from "../ipc/types";
import { getServers, ServerListItem } from "../api";
import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";

type Action = "install" | "update" | "play" | "blocked";

export function HomeTab() {
  const { settings, update, reload } = useSettings();
  const [detected, setDetected] = useState<DetectedInstall[] | null>(null);
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
  const [serverStats, setServerStats] = useState<{
    servers: number;
    players: number;
    regions: number;
  } | null>(null);
  // Flips true once the auto-adopt effect has finished (adopted or not).
  // We use this to suppress the onboarding card until we know whether the
  // backend will populate `library_root` from a detected official install.
  const [autoAdoptChecked, setAutoAdoptChecked] = useState(false);
  // Onboarding wizard state
  const [wizardStep, setWizardStep] = useState(0); // 0=dir, 1=download, 2=import, 3=verify
  const [wizardDismissed, setWizardDismissed] = useState(false);
  const [wizardSelectedPath, setWizardSelectedPath] = useState<string | null>(
    null,
  );
  const [diskSuggestions, setDiskSuggestions] = useState<
    DiskSuggestion[] | null
  >(null);
  const [copyFlash, setCopyFlash] = useState(false);
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

  // Fetch server stats for the hero card.
  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const data = await getServers();
        if (cancelled) return;
        const players = data.reduce(
          (s: number, sv: ServerListItem) => s + sv.player_count,
          0,
        );
        const regions = new Set(
          data.map((s: ServerListItem) => s.region).filter(Boolean),
        ).size;
        setServerStats({ servers: data.length, players, regions });
      } catch {
        /* silent */
      }
    };
    load();
    const t = setInterval(load, 30_000);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, []);

  // Auto-select the default channel if none is set.
  useEffect(() => {
    if (!settings?.mirror_domain || settings.selected_channel) return;
    update({ selected_channel: "live_game" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings?.mirror_domain, settings?.selected_channel]);

  // Check for updates when channel/install state changes.
  useEffect(() => {
    if (!settings?.selected_channel || !settings?.mirror_domain) {
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
  }, [settings?.selected_channel, settings?.channels, settings?.mirror_domain]);

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
    if (!settings.mirror_domain || !settings.library_root) {
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
          const pid = await launchGame(
            channel,
            sel,
            launchableDir ?? undefined,
          );
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

  // Show the progress panel as soon as a job starts, even before the first
  // progress event matching it arrives. Without this there's a visible gap
  // after dialog close where nothing appears to happen (especially on
  // Windows when the zip crate is scanning the central directory), which
  // looks to the user like the buttons did nothing.
  const showingProgress = !!activeJobId;
  const matchedProgress: ProgressEvent | null =
    progress && progress.job_id === activeJobId ? progress : null;
  const placeholderProgress: ProgressEvent = {
    job_id: activeJobId ?? "",
    phase: { phase: "preparing" },
    file_index: 0,
    file_count: 0,
    bytes_done: 0,
    bytes_total: 0,
    current_file: "",
    speed_bps: 0,
    eta_seconds: 0,
  };
  const displayProgress = matchedProgress ?? placeholderProgress;

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

  // The onboarding wizard shows when: auto-adopt is done, no detected
  // launchable install, and the game is not installed via our launcher.
  const needsOnboarding =
    !!settings && autoAdoptChecked && !launchableDetected && !installed;

  // Fetch disk suggestions when entering the wizard.
  useEffect(() => {
    if (!needsOnboarding) return;
    suggestInstallPath()
      .then(setDiskSuggestions)
      .catch(() => setDiskSuggestions([]));
  }, [needsOnboarding]);

  // Derive the detected install's library_root (if any).
  const detectedRoot = (() => {
    if (!detected || detected.length === 0) return null;
    // Prefer a hit that has r5apex.exe and contains "R5R Library" in path.
    for (const d of detected) {
      const segs = d.path.replace(/\\/g, "/").split("/");
      const libIdx = segs.findIndex((s) => s.toLowerCase() === "r5r library");
      if (libIdx > 0) return segs.slice(0, libIdx).join("\\");
    }
    // Fallback: registry/shortcut hit points at the base install dir
    // (e.g. "D:\Program Files\R5Reloaded") — use it directly as library_root.
    const first = detected[0];
    if (first) return first.path.replace(/[\\/]+$/, "");
    return null;
  })();

  // Pre-select: prefer detected install, else first disk with enough space.
  useEffect(() => {
    if (needsOnboarding && wizardStep === 0 && !wizardSelectedPath) {
      if (detectedRoot) {
        setWizardSelectedPath(detectedRoot.replace(/\\/g, "/").replace(/\/+$/, ""));
      } else if (diskSuggestions && diskSuggestions.length > 0) {
        const ok = diskSuggestions.find(
          (d) => d.free_bytes >= 30 * 1024 * 1024 * 1024,
        );
        if (ok) {
          const p = ok.path.replace(/[\\/]+$/, "").replace(/\\/g, "/");
          setWizardSelectedPath(p + "/R5Reloaded");
        }
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needsOnboarding, wizardStep, detectedRoot, diskSuggestions]);

  // Auto-advance wizard: import complete → step 3 (verify) auto-trigger.
  useEffect(() => {
    if (
      needsOnboarding &&
      wizardStep === 2 &&
      progress?.job_id === activeJobId &&
      progress.phase.phase === "complete"
    ) {
      // Import done → auto-trigger verification.
      setWizardStep(3);
      const channel = settings?.selected_channel || "live_game";
      startRepair(channel)
        .then((id) => beginJob(id, true))
        .catch(() => {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needsOnboarding, wizardStep, progress?.phase]);

  const handleConfirmPath = async () => {
    if (!wizardSelectedPath) return;
    await update({ library_root: wizardSelectedPath });
    setWizardStep(1);
  };

  const handleBrowseFolder = async () => {
    const picked = await openDialog({
      directory: true,
      multiple: false,
      title: "选择安装根目录（建议 30GB 以上可用空间）",
    });
    if (typeof picked === "string") {
      setWizardSelectedPath(picked);
    }
  };

  const handleCopyUrl = async (url: string) => {
    await navigator.clipboard.writeText(url);
    setCopyFlash(true);
    window.setTimeout(() => setCopyFlash(false), 1500);
  };

  const handleWizardImportZip = async () => {
    setImportError(null);
    if (!settings?.library_root) {
      setImportError("请先在第 1 步选择安装目录");
      return;
    }
    try {
      const picked = await openDialog({
        directory: false,
        multiple: false,
        title: "选择已下载的离线包 zip",
        filters: [{ name: "Zip", extensions: ["zip"] }],
      });
      if (typeof picked !== "string") return;
      const id = await startOfflineImport(settings.library_root, {
        type: "zip",
        path: picked,
      });
      beginJob(id, false);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleWizardImportDir = async () => {
    setImportError(null);
    if (!settings?.library_root) {
      setImportError("请先在第 1 步选择安装目录");
      return;
    }
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "选择已解压的离线包目录",
      });
      if (typeof picked !== "string") return;
      const id = await startOfflineImport(settings.library_root, {
        type: "directory",
        path: picked,
      });
      beginJob(id, false);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    }
  };

  const formatGB = (bytes: number) =>
    `${(bytes / 1024 / 1024 / 1024).toFixed(0)} GB`;

  /** Normalize path separators to forward slashes and strip trailing slash. */
  const fwd = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");

  const channelDisplayName = (ch: string) => {
    switch (ch) {
      case "live_game": return "R5Reloaded Live";
      default: return ch;
    }
  };

  const STEP_LABELS = ["选择目录", "下载离线包", "导入安装", "校验文件"];

  return (
    <div className="p-6 space-y-5">
      {needsOnboarding && !wizardDismissed && (
        <GlassCard className="border-blue-400/40 bg-blue-500/[0.08]">
          {/* Step indicator */}
          <div className="flex items-center gap-2 mb-4">
            {STEP_LABELS.map((label, i) => (
              <div key={label} className="flex items-center gap-2">
                {i > 0 && <div className="w-6 h-px bg-white/20" />}
                <div className="flex items-center gap-1.5">
                  <span
                    className={`size-5 rounded-full text-[10px] flex items-center justify-center font-bold ${
                      i < wizardStep
                        ? "bg-emerald-400/80 text-black"
                        : i === wizardStep
                          ? "bg-blue-400/80 text-black"
                          : "bg-white/10 text-white/40"
                    }`}
                  >
                    {i < wizardStep ? "✓" : i + 1}
                  </span>
                  <span
                    className={`text-[11px] ${
                      i === wizardStep
                        ? "text-blue-200 font-medium"
                        : "text-white/40"
                    }`}
                  >
                    {label}
                  </span>
                </div>
              </div>
            ))}
          </div>

          {/* Step 0: Pick install directory */}
          {wizardStep === 0 && (
            <div className="space-y-3">
              <div className="text-sm font-semibold text-blue-100">
                选择游戏安装目录
              </div>
              <div className="text-xs text-blue-100/70 leading-relaxed">
                请选择安装路径，需要至少 30GB 可用空间。建议使用 C
                盘以外的磁盘。
              </div>

              {diskSuggestions === null && (
                <div className="text-xs text-white/40">正在扫描磁盘…</div>
              )}

              {diskSuggestions && (
                <div className="space-y-1.5">
                  <div className="text-[11px] text-white/40 uppercase tracking-wider">
                    可用安装路径
                  </div>
                  {diskSuggestions
                    .filter((d) => {
                      if (!detectedRoot) return true;
                      const diskRoot = fwd(d.path.replace(/[\\/]+$/, "") + "/R5Reloaded");
                      return diskRoot.toLowerCase() !== fwd(detectedRoot).toLowerCase();
                    })
                    .map((d) => {
                      const MIN = 30 * 1024 * 1024 * 1024;
                      const enough = d.free_bytes >= MIN;
                      const rootPath = fwd(d.path.replace(/[\\/]+$/, "") + "/R5Reloaded");
                      const isSelected = wizardSelectedPath === rootPath;
                      return (
                        <button
                          key={d.path}
                          type="button"
                          disabled={!enough}
                          onClick={() => enough && setWizardSelectedPath(rootPath)}
                          className={`w-full flex items-center justify-between px-3 py-2 rounded-lg border transition-all text-left ${
                            !enough
                              ? "border-white/5 bg-white/[0.01] opacity-50 cursor-not-allowed"
                              : isSelected
                                ? "border-blue-400/60 bg-blue-400/10"
                                : "border-white/10 bg-white/[0.03] hover:bg-white/[0.08] hover:border-blue-400/40"
                          }`}
                        >
                          <span className="font-mono text-xs text-white/80">
                            {rootPath}
                          </span>
                          <span
                            className={`text-xs ml-2 shrink-0 ${enough ? "text-emerald-300" : "text-red-300"}`}
                          >
                            {formatGB(d.free_bytes)} 可用
                          </span>
                        </button>
                      );
                    })}

                  {/* Detected R5Reloaded install (shown last, pre-selected) */}
                  {detectedRoot && (
                    <button
                      type="button"
                      onClick={() => setWizardSelectedPath(fwd(detectedRoot))}
                      className={`w-full flex items-center justify-between px-3 py-2 rounded-lg border transition-all text-left ${
                        wizardSelectedPath === fwd(detectedRoot)
                          ? "border-emerald-400/60 bg-emerald-400/10"
                          : "border-white/10 bg-white/[0.03] hover:bg-white/[0.08] hover:border-emerald-400/40"
                      }`}
                    >
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-xs text-white/80">
                          {fwd(detectedRoot)}
                        </span>
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-500/15 text-emerald-300 shrink-0">
                          已安装
                        </span>
                      </div>
                    </button>
                  )}
                </div>
              )}

              {/* Install dir preview */}
              {wizardSelectedPath && (
                <div className="text-xs text-white/50">
                  实际安装目录：
                  <span className="font-mono text-white/70">
                    {fwd(wizardSelectedPath)}/R5R Library/&lt;频道&gt;/
                  </span>
                </div>
              )}

              <div className="flex gap-2">
                <PrimaryButton
                  variant="primary"
                  onClick={handleConfirmPath}
                  disabled={!wizardSelectedPath}
                >
                  确认选择
                </PrimaryButton>
                <PrimaryButton variant="secondary" onClick={handleBrowseFolder}>
                  手动选择…
                </PrimaryButton>
              </div>
            </div>
          )}

          {/* Step 1: Download offline package */}
          {wizardStep === 1 && (
            <div className="space-y-3">
              <div className="text-sm font-semibold text-blue-100">
                下载离线安装包
              </div>
              <div className="text-xs text-blue-100/70 leading-relaxed">
                游戏完整包约 30GB，建议通过离线包安装（比在线下载更快更稳定）。
                请复制下方链接或在浏览器中打开，下载完成后进入下一步。
              </div>
              {dashboard?.offline_package_url ? (
                <div className="space-y-2">
                  <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-white/[0.04] border border-white/10">
                    <span className="text-xs font-mono text-white/70 truncate flex-1">
                      {dashboard.offline_package_url}
                    </span>
                  </div>
                  <div className="flex gap-2">
                    <PrimaryButton
                      variant="secondary"
                      onClick={() =>
                        handleCopyUrl(dashboard.offline_package_url)
                      }
                    >
                      {copyFlash ? "已复制 ✓" : "复制链接"}
                    </PrimaryButton>
                    <PrimaryButton
                      variant="primary"
                      onClick={() =>
                        openExternalUrl(dashboard.offline_package_url)
                      }
                    >
                      在浏览器中打开
                    </PrimaryButton>
                  </div>
                </div>
              ) : (
                <div className="text-xs text-amber-300">
                  离线包地址暂未配置，请联系管理员或直接跳过使用在线安装。
                </div>
              )}
              <div className="flex items-center gap-2 pt-1">
                <PrimaryButton
                  variant="primary"
                  onClick={() => setWizardStep(2)}
                >
                  已下载，进入下一步
                </PrimaryButton>
                <PrimaryButton
                  variant="secondary"
                  onClick={() => setWizardDismissed(true)}
                >
                  跳过
                </PrimaryButton>
              </div>
            </div>
          )}

          {/* Step 2: Import offline package (zip or directory) */}
          {wizardStep === 2 && (
            <div className="space-y-3">
              <div className="text-sm font-semibold text-blue-100">
                导入离线安装包
              </div>
              <div className="text-xs text-blue-100/70 leading-relaxed">
                选择已下载的离线包（支持 zip 压缩包或已解压的目录）。
              </div>
              {showingProgress ? (
                <InstallProgress
                  progress={displayProgress}
                  logs={installLogs}
                  onCancel={handleCancelImport}
                  paused={jobPaused}
                />
              ) : (
                <div className="flex gap-2">
                  <PrimaryButton
                    variant="primary"
                    onClick={handleWizardImportZip}
                  >
                    选择离线包 zip
                  </PrimaryButton>
                  <PrimaryButton
                    variant="secondary"
                    onClick={handleWizardImportDir}
                  >
                    选择已解压的目录
                  </PrimaryButton>
                </div>
              )}
              {importError && (
                <div className="text-xs text-red-300">
                  导入失败：{importError}
                </div>
              )}
            </div>
          )}

          {/* Step 3: Verify */}
          {wizardStep === 3 && (
            <div className="space-y-3">
              <div className="text-sm font-semibold text-blue-100">
                校验游戏文件
              </div>
              <div className="text-xs text-blue-100/70 leading-relaxed">
                正在校验文件完整性，确保所有游戏文件与服务器一致。
              </div>
              {showingProgress ? (
                <InstallProgress
                  progress={displayProgress}
                  logs={installLogs}
                  onCancel={handleCancelImport}
                  onTogglePause={
                    activeJobPausable ? handleTogglePause : undefined
                  }
                  paused={jobPaused}
                />
              ) : (
                <div className="text-xs text-emerald-300">
                  校验完成，可以开始游戏了！
                </div>
              )}
            </div>
          )}
        </GlassCard>
      )}

      {/* Non-forced update banner — shown at the top, dismissible. */}

      <GlassCard
        className="relative overflow-hidden"
        padding={false}
      >
        <div className="absolute inset-0 bg-gradient-to-tr from-blue-500/10 via-transparent to-purple-500/10" />
        <div className="relative p-8 flex flex-col">
          <div>
            <div className="text-3xl font-bold tracking-tight">
              R5R 中国镜像启动器
            </div>
            <div className="text-white/60 mt-2 max-w-xl">
              社区服专用 · 镜像加速 · 一键启动
              {serverStats && (
                <span className="ml-3 text-white/50">
                  {serverStats.servers} 服务器 · {serverStats.players} 在线玩家
                  · {serverStats.regions} 地区
                </span>
              )}
            </div>

            {settings?.mirror_domain && (
              <div className="mt-6 flex items-center gap-2 flex-wrap text-[11px] tabular-nums">
                {/* Channel pill */}
                <span className="flex items-center gap-1.5 px-2.5 py-1 rounded-md border border-blue-400/40 bg-blue-400/10 text-white font-medium">
                  {installed ? (
                    <span className="size-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.7)]" />
                  ) : (
                    <span className="size-1.5 rounded-full border border-white/40" />
                  )}
                  <span>
                    {channelDisplayName(settings?.selected_channel || "live_game")}
                  </span>
                </span>

                {/* Version badges — all inline */}
                {installed && (
                  <span className="flex items-center gap-1 px-2 py-1 rounded-md bg-emerald-500/10 text-emerald-300 border border-emerald-400/20 font-mono">
                    <span className="text-white/45 font-sans">本地</span>
                    {settings.channels[settings.selected_channel]?.version ||
                      "—"}
                  </span>
                )}
                {remoteVersion && (
                  <span className="flex items-center gap-1 px-2 py-1 rounded-md bg-white/[0.04] text-white/65 border border-white/10 font-mono">
                    <span className="text-white/40 font-sans">远端</span>
                    {remoteVersion}
                  </span>
                )}
                {dashboard?.game_version && (
                  <span className="flex items-center gap-1 px-2 py-1 rounded-md bg-blue-500/10 text-blue-300 border border-blue-400/20 font-mono">
                    <span className="text-white/45 font-sans">社区服</span>
                    {dashboard.game_version}
                  </span>
                )}
                {installed && updateAvailable && (
                  <span className="flex items-center gap-1 px-2 py-1 rounded-md bg-amber-500/15 text-amber-300 border border-amber-400/30">
                    ↻ 有更新
                  </span>
                )}
                {installed && updateAvailable === false && (
                  <span className="flex items-center gap-1 px-2 py-1 rounded-md bg-emerald-500/10 text-emerald-300/80 border border-emerald-400/20">
                    ✓ 已是最新
                  </span>
                )}
              </div>
            )}
          </div>

          {showingProgress ? (
            <div className="mt-4">
              <InstallProgress
                progress={displayProgress}
                logs={installLogs}
                onCancel={handleCancelImport}
                onTogglePause={
                  activeJobPausable ? handleTogglePause : undefined
                }
                paused={jobPaused}
              />
            </div>
          ) : (
            <div className="space-y-3 mt-4">
              <div className="flex items-center gap-3 flex-wrap">
                <PrimaryButton
                  variant={actionVariant(action)}
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
                  disabled={
                    !settings?.selected_channel || !settings?.library_root
                  }
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
                <div className="text-xs text-red-300">
                  启动失败：{launchError}
                </div>
              )}
              {importError && (
                <div className="text-xs text-red-300">
                  操作失败：{importError}
                </div>
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

      <GlassCard>
        <SectionHeader
          icon="📣"
          title={dashboard?.announcement?.title || "公告"}
        />
        <div className="text-sm text-white/75 whitespace-pre-wrap leading-relaxed">
          {dashboard?.announcement?.content || (
            <span className="text-white/40">暂无公告。</span>
          )}
        </div>
        <div className="mt-4 flex flex-wrap gap-2">
          {dashboard?.docs_url && (
            <PrimaryButton
              variant="secondary"
              onClick={() => openExternalUrl(dashboard.docs_url)}
            >
              📖 查看文档
            </PrimaryButton>
          )}
          {dashboard?.offline_package_url && (
            <PrimaryButton
              variant="secondary"
              onClick={() => openExternalUrl(dashboard.offline_package_url)}
            >
              📦 离线包下载
            </PrimaryButton>
          )}
          <PrimaryButton
            variant="secondary"
            onClick={() => openExternalUrl("https://qm.qq.com/q/cJb0EBg7Dy")}
          >
            💬 加入QQ群
          </PrimaryButton>
        </div>
      </GlassCard>

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

    </div>
  );
}
