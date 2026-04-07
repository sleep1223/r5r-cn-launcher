import { useEffect, useMemo, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { InstallProgress } from "../components/InstallProgress";
import { useSettings } from "../hooks/useSettings";
import { useLaunchExited } from "../hooks/useLaunchExited";
import { useInstallProgress } from "../hooks/useInstallProgress";
import { detectExistingR5R } from "../ipc/detect";
import { fetchRemoteConfig } from "../ipc/config";
import { launchGame } from "../ipc/launch";
import {
  cancelInstall,
  checkUpdate,
  startOfflineImport,
  startOnlineInstall,
  startRepair,
  startUpdate,
} from "../ipc/install";
import {
  DetectedInstall,
  LaunchOptionSelection,
  RemoteConfig,
} from "../ipc/types";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

type Action = "install" | "update" | "play" | "blocked";

export function HomeTab() {
  const { settings, update, reload } = useSettings();
  const [detected, setDetected] = useState<DetectedInstall[] | null>(null);
  const [config, setConfig] = useState<RemoteConfig | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchedPid, setLaunchedPid] = useState<number | null>(null);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const [updateAvailable, setUpdateAvailable] = useState<boolean | null>(null);
  const [remoteVersion, setRemoteVersion] = useState<string | null>(null);
  const exited = useLaunchExited();
  const progress = useInstallProgress();

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
      window.setTimeout(() => setActiveJobId(null), 1500);
    }
  }, [progress, activeJobId, reload]);

  const installed =
    !!settings &&
    !!settings.selected_channel &&
    !!settings.channels[settings.selected_channel]?.installed;

  const action: Action = useMemo(() => {
    if (!settings) return "blocked";
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
  }, [settings, detected, installed, updateAvailable]);

  const launchableDir =
    !installed && detected && detected.length > 0 ? detected[0].path : null;

  const handlePrimaryAction = async () => {
    if (!settings) return;
    setImportError(null);
    setLaunchError(null);
    setLaunchedPid(null);

    try {
      switch (action) {
        case "install": {
          const id = await startOnlineInstall(settings.selected_channel);
          setActiveJobId(id);
          break;
        }
        case "update": {
          const id = await startUpdate(settings.selected_channel);
          setActiveJobId(id);
          break;
        }
        case "play": {
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
      setActiveJobId(id);
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
      setActiveJobId(id);
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
      setActiveJobId(id);
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCancelImport = async () => {
    if (!activeJobId) return;
    await cancelInstall(activeJobId);
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

  return (
    <div className="p-6 space-y-5">
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
              <div className="mt-6 flex items-center gap-3 flex-wrap">
                <span className="text-xs text-white/50">频道：</span>
                <select
                  value={settings?.selected_channel ?? ""}
                  onChange={(e) => update({ selected_channel: e.target.value })}
                  className="!w-auto"
                >
                  {config.channels.map((c) => (
                    <option key={c.name} value={c.name} disabled={!c.enabled}>
                      {c.name} {!c.enabled && "（已禁用）"}
                    </option>
                  ))}
                </select>
                {refreshing && (
                  <span className="text-xs text-white/40">刷新中…</span>
                )}
                {installed && (
                  <span className="text-xs text-emerald-300">
                    本地版本：
                    {settings.channels[settings.selected_channel]?.version ||
                      "—"}
                  </span>
                )}
                {remoteVersion && (
                  <span className="text-xs text-white/40">
                    远端：{remoteVersion}
                  </span>
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
              <InstallProgress progress={progress!} onCancel={handleCancelImport} />
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
                {installed && (
                  <PrimaryButton variant="secondary" onClick={handleRepair}>
                    校验并修复
                  </PrimaryButton>
                )}
              </div>
              {launchableDir && action === "play" && !installed && (
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
