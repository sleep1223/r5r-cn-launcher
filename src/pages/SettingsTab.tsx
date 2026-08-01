import { useEffect, useMemo, useRef, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { useSettings } from "../hooks/useSettings";
import {
  DiagnosticReportResult,
  DetectedInstall,
  ProxyMode,
  ProxyTestResult,
  UpdateStrategy,
} from "../ipc/types";
import { setProxyMode, testProxy } from "../ipc/proxy";
import {
  collectCrashDiagnostics,
  openConfigFolder,
  openDiagnosticReportFolder,
  openLogFolder,
  validateInstallPath,
} from "../ipc/settings";
import { detectExistingR5R } from "../ipc/detect";
import {
  open as openDialog,
  save as saveDialog,
} from "@tauri-apps/plugin-dialog";

const CUSTOM_OPTION = "__custom__";
const DOWNLOAD_CONCURRENCY_OPTIONS = [1, 4, 5, 10, 15, 20, 50, 75, 100];
const GAME_LANGUAGES = [
  ["schinese", "简体中文"],
  ["tchinese", "繁体中文"],
  ["english", "English"],
  ["japanese", "日本語"],
  ["korean", "한국어"],
  ["german", "Deutsch"],
  ["french", "Français"],
  ["italian", "Italiano"],
  ["spanish", "Español"],
  ["portuguese", "Português"],
  ["polish", "Polski"],
  ["russian", "Русский"],
] as const;

interface SettingsTabProps {
  focusDiagnostics?: boolean;
}

/**
 * Walk up `detectedPath` looking for the `R5R Library` segment and return its
 * parent — that's what we need to put into `settings.library_root`. The
 * detected path itself is the *channel* dir (e.g. `C:\R5R Library\LIVE`).
 */
function detectedToLibraryRoot(detectedPath: string): string {
  const segs = detectedPath.split(/[\\/]/);
  for (let i = segs.length - 1; i > 0; i--) {
    if (segs[i].toLowerCase() === "r5r library") {
      const parent = segs.slice(0, i).join("/");
      return /^[a-z]:$/i.test(parent) ? parent + "/" : parent;
    }
  }
  return detectedPath.replace(/\\/g, "/");
}

interface DetectedRoot {
  libraryRoot: string;
  detectedPath: string;
  channel: string | null;
  source: DetectedInstall["source"];
}

/**
 * Render the install root + the fixed `R5R Library/<channel>/` suffix using a
 * single forward-slash separator regardless of how the user typed the path.
 * The backend always builds the install dir as `<root>/R5R Library/<CHANNEL>/`
 * — the user just wants a tidy preview.
 */
function formatInstallDirPreview(root: string): string {
  if (!root) return "<未选择>/R5R Library/<频道>/";
  const normalized = root.replace(/[\\/]+/g, "/").replace(/\/+$/, "");
  return `${normalized}/R5R Library/<频道>/`;
}

export function SettingsTab({ focusDiagnostics = false }: SettingsTabProps) {
  const { settings, loading, error, update, reload } = useSettings();
  const [proxyKind, setProxyKind] = useState<ProxyMode["kind"]>("system");
  const [proxyUrl, setProxyUrl] = useState("");
  const [mirrorDomain, setMirrorDomain] = useState("");
  const [updateStrategy, setUpdateStrategy] = useState<UpdateStrategy>("patch");
  const [downloadHdTextures, setDownloadHdTextures] = useState(false);
  const [installedLanguages, setInstalledLanguages] = useState<string[]>([
    "schinese",
  ]);
  const [launchViaEaApp, setLaunchViaEaApp] = useState(true);
  const [usageReportingEnabled, setUsageReportingEnabled] = useState(true);
  const [libraryRoot, setLibraryRoot] = useState("");
  // Which row in the install-location dropdown is selected. Either a detected
  // library_root, or `__custom__` to enable the manual text input.
  const [installSelection, setInstallSelection] = useState<string>(CUSTOM_OPTION);
  const [concurrency, setConcurrency] = useState(4);
  const [pathErrors, setPathErrors] = useState<string[]>([]);
  const [pathWarnings, setPathWarnings] = useState<string[]>([]);
  const [proxyResult, setProxyResult] = useState<ProxyTestResult | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [detected, setDetected] = useState<DetectedInstall[] | null>(null);
  const [diagnosticResult, setDiagnosticResult] =
    useState<DiagnosticReportResult | null>(null);
  const [diagnosticError, setDiagnosticError] = useState<string | null>(null);
  const diagnosticsRef = useRef<HTMLDivElement>(null);
  // Tracks whether local state has been hydrated from `settings` at least
  // once. Until that flips true the autosave effect must not fire — we'd
  // otherwise immediately overwrite the saved settings with empty defaults.
  const hydrated = useRef(false);

  // Hydrate local form state from settings on first load. Also runs after a
  // manual `重置` (which calls `reload()`) to snap the UI back to disk.
  useEffect(() => {
    if (!settings) return;
    setProxyKind(settings.proxy_mode.kind);
    setProxyUrl(
      settings.proxy_mode.kind === "custom" ? settings.proxy_mode.url : "",
    );
    setMirrorDomain(settings.mirror_domain);
    setUpdateStrategy(settings.update_strategy);
    setDownloadHdTextures(settings.download_hd_textures);
    const selectedChannel = settings.selected_channel || "LIVE";
    const channelName =
      selectedChannel.toUpperCase() === "LIVE_GAME"
        ? "LIVE"
        : selectedChannel;
    const channelState =
      settings.channels[channelName] ??
      settings.channels[channelName.toUpperCase()] ??
      settings.channels.LIVE;
    setInstalledLanguages(
      channelState?.installed_languages.length
        ? channelState.installed_languages
        : ["schinese"],
    );
    setLaunchViaEaApp(settings.launch_via_ea_app);
    setUsageReportingEnabled(settings.usage_reporting_enabled);
    setLibraryRoot(settings.library_root.replace(/\\/g, "/"));
    setConcurrency(settings.concurrent_downloads);
    hydrated.current = true;
  }, [settings]);

  useEffect(() => {
    if (!focusDiagnostics || loading) return;
    const frame = window.requestAnimationFrame(() => {
      diagnosticsRef.current?.scrollIntoView({
        behavior: "smooth",
        block: "start",
      });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [focusDiagnostics, loading]);

  // Run detection once so we can offer detected installs as quick-pick options.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const r = await detectExistingR5R();
        if (!cancelled) setDetected(r);
      } catch {
        if (!cancelled) setDetected([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Dedupe detected installs by library_root so two channels under the same
  // root don't show up as duplicate dropdown entries.
  const detectedRoots: DetectedRoot[] = useMemo(() => {
    if (!detected) return [];
    const seen = new Set<string>();
    const out: DetectedRoot[] = [];
    for (const d of detected) {
      const root = detectedToLibraryRoot(d.path);
      const key = root.toLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({
        libraryRoot: root,
        detectedPath: d.path,
        channel: d.channel,
        source: d.source,
      });
    }
    return out;
  }, [detected]);

  // Once detection lands, decide whether the saved library_root matches a
  // detected entry (so the dropdown highlights it) or whether the user is on
  // a custom path.
  useEffect(() => {
    if (detected === null) return;
    const norm = (s: string) => s.replace(/\\/g, "/").toLowerCase();
    const match = detectedRoots.find(
      (r) => norm(r.libraryRoot) === norm(libraryRoot),
    );
    setInstallSelection(match ? match.libraryRoot : CUSTOM_OPTION);
    // Only run when detection finishes / settings hydrate; avoid stomping on
    // the user mid-edit by leaving libraryRoot out of the deps.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detected, settings?.library_root]);

  // Live-validate the install path as the user types.
  useEffect(() => {
    let cancelled = false;
    if (!libraryRoot) {
      setPathErrors([]);
      setPathWarnings([]);
      return;
    }
    const t = setTimeout(async () => {
      try {
        const r = await validateInstallPath(libraryRoot);
        if (!cancelled) {
          setPathErrors(r.errors);
          setPathWarnings(r.warnings);
        }
      } catch {
        /* ignore */
      }
    }, 200);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [libraryRoot]);

  if (loading) return <div className="p-8 text-white/60">加载中…</div>;
  if (error)
    return (
      <div className="p-8 text-red-400">加载设置失败：{error}</div>
    );
  if (!settings) return null;

  const buildProxyMode = (): ProxyMode => {
    if (proxyKind === "system") return { kind: "system" };
    if (proxyKind === "none") return { kind: "none" };
    return { kind: "custom", url: proxyUrl.trim() };
  };

  const handleTestProxy = async () => {
    setBusy("test");
    setProxyResult(null);
    try {
      const r = await testProxy(mirrorDomain.trim() || undefined);
      setProxyResult(r);
    } catch (e) {
      setProxyResult({
        ok: false,
        status: null,
        latency_ms: 0,
        error: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusy(null);
    }
  };

  const handlePickFolder = async () => {
    const picked = await openDialog({
      directory: true,
      multiple: false,
      title: "选择安装根目录",
    });
    if (typeof picked === "string") {
      setLibraryRoot(picked);
      setInstallSelection(CUSTOM_OPTION);
    }
  };

  const handleInstallSelect = (value: string) => {
    setInstallSelection(value);
    if (value !== CUSTOM_OPTION) {
      setLibraryRoot(value);
    }
  };

  // Autosave: whenever any local form state diverges from `settings`, persist
  // it after a short debounce. Skip until we've hydrated at least once and
  // until any path errors are resolved (to avoid persisting an invalid path).
  useEffect(() => {
    if (!hydrated.current || !settings) return;
    if (pathErrors.length > 0) return;

    const nextProxy = buildProxyMode();
    const trimmedDomain = mirrorDomain.trim();

    const proxyChanged =
      JSON.stringify(nextProxy) !== JSON.stringify(settings.proxy_mode);
    const domainChanged = trimmedDomain !== settings.mirror_domain;
    const libraryRootChanged = libraryRoot !== settings.library_root;
    const concurrencyChanged = concurrency !== settings.concurrent_downloads;
    const updateStrategyChanged = updateStrategy !== settings.update_strategy;
    const hdTexturesChanged =
      downloadHdTextures !== settings.download_hd_textures;
    const selectedChannel = settings.selected_channel || "LIVE";
    const channelName =
      selectedChannel.toUpperCase() === "LIVE_GAME"
        ? "LIVE"
        : selectedChannel;
    const currentChannel = settings.channels[channelName] ?? {
      installed: false,
      version: "",
      key: "",
      installed_languages: [],
    };
    const normalizedLanguages = [...installedLanguages].sort();
    const savedLanguages = currentChannel.installed_languages.length
      ? currentChannel.installed_languages
      : ["schinese"];
    const languagesChanged =
      JSON.stringify(normalizedLanguages) !==
      JSON.stringify([...savedLanguages].sort());
    const launchViaEaAppChanged =
      launchViaEaApp !== settings.launch_via_ea_app;
    const usageReportingChanged =
      usageReportingEnabled !== settings.usage_reporting_enabled;

    if (
      !proxyChanged &&
      !domainChanged &&
      !libraryRootChanged &&
      !concurrencyChanged &&
      !updateStrategyChanged &&
      !hdTexturesChanged &&
      !languagesChanged &&
      !launchViaEaAppChanged &&
      !usageReportingChanged
    ) {
      return;
    }

    const handle = window.setTimeout(async () => {
      setBusy("autosave");
      setSaveError(null);
      try {
        if (proxyChanged) {
          await setProxyMode(nextProxy);
        }
        await update({
          proxy_mode: nextProxy,
          mirror_domain: trimmedDomain,
          library_root: libraryRoot,
          concurrent_downloads: concurrency,
          update_strategy: updateStrategy,
          download_hd_textures: downloadHdTextures,
          channels: languagesChanged
            ? {
                ...settings.channels,
                [channelName]: {
                  ...currentChannel,
                  installed_languages: normalizedLanguages,
                },
              }
            : settings.channels,
          launch_via_ea_app: launchViaEaApp,
          usage_reporting_enabled: usageReportingEnabled,
        });
        setSavedAt(Date.now());
      } catch (e) {
        setSaveError(e instanceof Error ? e.message : String(e));
      } finally {
        setBusy(null);
      }
    }, 400);

    return () => window.clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    proxyKind,
    proxyUrl,
    mirrorDomain,
    updateStrategy,
    downloadHdTextures,
    installedLanguages,
    launchViaEaApp,
    usageReportingEnabled,
    libraryRoot,
    concurrency,
    pathErrors.length,
  ]);

  const handleReset = async () => {
    setBusy("reset");
    setSaveError(null);
    // Mark as un-hydrated so the autosave effect doesn't fire with stale
    // local state before the hydration effect re-runs with fresh settings.
    hydrated.current = false;
    try {
      await reload();
      setSavedAt(null);
    } finally {
      setBusy(null);
    }
  };

  const handleCollectDiagnostics = async () => {
    const timestamp = new Date()
      .toISOString()
      .replace(/[:.]/g, "-")
      .replace("T", "_")
      .replace("Z", "");
    const destination = await saveDialog({
      title: "保存 R5R 崩溃诊断包",
      defaultPath: `r5r-crash-report-${timestamp}.zip`,
      filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
    });
    if (!destination) return;

    setBusy("diagnostics");
    setDiagnosticResult(null);
    setDiagnosticError(null);
    try {
      setDiagnosticResult(await collectCrashDiagnostics(destination));
    } catch (e) {
      setDiagnosticError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="max-w-3xl mx-auto py-6 px-6 space-y-5">
      {/* 网络代理 */}
      <GlassCard>
        <SectionHeader
          icon="🌐"
          title="网络代理"
          subtitle="选择启动器访问网络时使用的代理。"
        />
        <div className="space-y-3">
          <div className="flex gap-2">
            {(["system", "custom", "none"] as const).map((k) => (
              <button
                key={k}
                onClick={() => setProxyKind(k)}
                className={`flex-1 py-2 rounded-lg border text-sm transition-all ${
                  proxyKind === k
                    ? "border-blue-400/60 bg-blue-400/10 text-white"
                    : "border-white/10 text-white/60 hover:bg-white/5"
                }`}
              >
                {k === "system" && "系统代理"}
                {k === "custom" && "自定义"}
                {k === "none" && "不使用"}
              </button>
            ))}
          </div>
          {proxyKind === "custom" && (
            <input
              type="text"
              placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080"
              value={proxyUrl}
              onChange={(e) => setProxyUrl(e.target.value)}
            />
          )}
          <div className="flex gap-2">
            <PrimaryButton
              variant="secondary"
              onClick={handleTestProxy}
              disabled={busy === "test"}
            >
              {busy === "test" ? "测试中…" : "测试连通性"}
            </PrimaryButton>
            <span className="text-xs text-white/40 self-center">
              未设置镜像源时测试官方 CDN。
            </span>
          </div>
          {proxyResult && (
            <div
              className={`text-sm px-3 py-2 rounded-lg ${
                proxyResult.ok
                  ? "bg-emerald-500/10 text-emerald-300"
                  : "bg-red-500/10 text-red-300"
              }`}
            >
              {proxyResult.ok
                ? `连接成功 · HTTP ${proxyResult.status} · ${proxyResult.latency_ms} ms`
                : `失败：${proxyResult.error ?? "未知错误"} (${proxyResult.latency_ms} ms)`}
            </div>
          )}
        </div>
      </GlassCard>

      {/* 镜像源 */}
      <GlassCard>
        <SectionHeader
          icon="🪞"
          title="镜像源"
          subtitle="用于获取版本和校验信息；留空则使用官方 CDN。"
        />
        <input
          type="text"
          placeholder="cdn-r5r-org.sleep0.de"
          value={mirrorDomain}
          onChange={(e) => setMirrorDomain(e.target.value)}
        />
      </GlassCard>

      {/* 更新方式 */}
      <GlassCard>
        <SectionHeader
          icon="↻"
          title="更新方式"
          subtitle="选择游戏文件的更新方式。"
        />
        <div className="flex gap-2">
          {(["patch", "verify"] as const).map((s) => (
            <button
              key={s}
              type="button"
              onClick={() => setUpdateStrategy(s)}
              className={`flex-1 py-2 rounded-lg border text-sm transition-all ${
                updateStrategy === s
                  ? "border-blue-400/60 bg-blue-400/10 text-white"
                  : "border-white/10 text-white/60 hover:bg-white/5"
              }`}
            >
              {s === "patch" ? "补丁包（推荐）" : "完整校验"}
            </button>
          ))}
        </div>
        {updateStrategy === "patch" && (
          <div className="text-xs text-white/50 mt-2">
            优先下载版本差异，无法使用时自动校验文件。
          </div>
        )}
      </GlassCard>

      {/* HD 纹理 */}
      <GlassCard>
        <SectionHeader
          icon="🖼️"
          title="HD 高清纹理"
          subtitle="提供更清晰的纹理，需要更多磁盘空间和下载流量。"
        />
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={downloadHdTextures}
            onChange={(e) => setDownloadHdTextures(e.target.checked)}
            className="h-4 w-4 accent-blue-400"
          />
          <span className="text-sm text-white/80">下载 HD 高清纹理</span>
        </label>
      </GlassCard>

      {/* 游戏语言 */}
      <GlassCard>
        <SectionHeader
          icon="文"
          title="游戏语言包"
          subtitle="校验、下载和更新时保留所选语言文件；至少选择一种。"
        />
        <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
          {GAME_LANGUAGES.map(([code, label]) => {
            const checked = installedLanguages.includes(code);
            return (
              <label
                key={code}
                className="flex items-center gap-2 rounded-lg border border-white/10 px-3 py-2 text-sm text-white/75"
              >
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={checked && installedLanguages.length === 1}
                  onChange={(event) => {
                    setInstalledLanguages((current) =>
                      event.target.checked
                        ? [...new Set([...current, code])]
                        : current.filter((language) => language !== code),
                    );
                  }}
                  className="h-4 w-4 accent-blue-400"
                />
                <span>{label}</span>
              </label>
            );
          })}
        </div>
      </GlassCard>

      {/* EA App 随启动器启动 */}
      <GlassCard>
        <SectionHeader
          icon="🎮"
          title="自动打开 EA App"
          subtitle="打开启动器时同步启动 EA App。"
        />
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={launchViaEaApp}
            onChange={(e) => setLaunchViaEaApp(e.target.checked)}
            className="h-4 w-4 accent-blue-400"
          />
          <span className="text-sm text-white/80">启用自动打开</span>
        </label>
      </GlassCard>

      {/* 匿名使用统计 */}
      <GlassCard>
        <SectionHeader
          icon="◉"
          title="匿名使用统计"
          subtitle="帮助了解启动器的使用情况。"
        />
        <label className="flex items-center gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={usageReportingEnabled}
            onChange={(e) => setUsageReportingEnabled(e.target.checked)}
            className="h-4 w-4 accent-blue-400"
          />
          <span className="text-sm text-white/80">参与匿名统计</span>
        </label>
      </GlassCard>

      {/* 安装位置 */}
      <GlassCard>
        <SectionHeader
          icon="📁"
          title="安装位置"
          subtitle={`实际安装目录：${formatInstallDirPreview(libraryRoot)}`}
        />
        <div className="space-y-2">
          <select
            value={installSelection}
            onChange={(e) => handleInstallSelect(e.target.value)}
            className="w-full"
          >
            {detectedRoots.map((d) => (
              <option key={d.libraryRoot} value={d.libraryRoot}>
                {d.libraryRoot}
                {d.channel ? ` · ${d.channel}` : ""} · 已检测到的官方安装
              </option>
            ))}
            <option value={CUSTOM_OPTION}>
              {detectedRoots.length > 0 ? "自定义位置…" : "自定义位置"}
            </option>
          </select>

          {installSelection === CUSTOM_OPTION && (
            <div className="flex gap-2">
              <input
                type="text"
                placeholder="例如 D:\\Games"
                value={libraryRoot}
                onChange={(e) => setLibraryRoot(e.target.value)}
              />
              <PrimaryButton variant="secondary" onClick={handlePickFolder}>
                浏览…
              </PrimaryButton>
            </div>
          )}

          {detected !== null && detectedRoots.length === 0 && (
            <div className="text-xs text-white/40">
              未检测到已有安装
              {navigator.userAgent.includes("Mac") && "（macOS 不支持检测）"}
              ，请手动填写。
            </div>
          )}

          {pathErrors.map((e, i) => (
            <div
              key={`err-${i}`}
              className="text-xs px-3 py-2 rounded-lg bg-red-500/10 text-red-300"
            >
              ✗ {e}
            </div>
          ))}
          {pathWarnings.map((w, i) => (
            <div
              key={`warn-${i}`}
              className="text-xs px-3 py-2 rounded-lg bg-amber-500/10 text-amber-300"
            >
              ⚠ {w}
            </div>
          ))}
        </div>
      </GlassCard>

      {/* 下载 */}
      <GlassCard>
        <SectionHeader
          icon="⬇"
          title="下载并发数"
          subtitle="限制同时进行的 HTTP 下载请求；分块请求也计入，默认 4。"
        />
        <div className="flex items-center gap-3">
          <select
            value={concurrency}
            onChange={(e) => setConcurrency(Number(e.target.value))}
            className="w-full"
          >
            {!DOWNLOAD_CONCURRENCY_OPTIONS.includes(concurrency) && (
              <option value={concurrency}>{concurrency}（当前设置）</option>
            )}
            {DOWNLOAD_CONCURRENCY_OPTIONS.map((value) => (
              <option key={value} value={value}>
                {value}
                {value === 4 ? "（默认）" : value === 100 ? "（最高）" : ""}
              </option>
            ))}
          </select>
        </div>
      </GlassCard>

      {/* 高级 */}
      <div ref={diagnosticsRef} className="scroll-mt-6">
        <GlassCard>
          <SectionHeader
            icon="⚒"
            title="高级"
            subtitle="生成用于排查崩溃的诊断包，并保存到你选择的位置。"
          />
          <div className="flex flex-wrap gap-2">
            <PrimaryButton
              variant="secondary"
              onClick={handleCollectDiagnostics}
              disabled={busy === "diagnostics"}
            >
              {busy === "diagnostics" ? "正在收集…" : "收集崩溃日志"}
            </PrimaryButton>
            <PrimaryButton variant="secondary" onClick={() => openLogFolder()}>
              打开日志目录
            </PrimaryButton>
            <PrimaryButton
              variant="secondary"
              onClick={() => openConfigFolder()}
            >
              打开配置目录
            </PrimaryButton>
          </div>
          {diagnosticError && (
            <div className="mt-3 rounded-lg bg-red-500/10 px-3 py-2 text-sm text-red-300">
              收集失败：{diagnosticError}
            </div>
          )}
          {diagnosticResult && (
            <div
              className={`mt-3 rounded-lg px-3 py-3 text-sm ${
                diagnosticResult.risky_applications.length > 0
                  ? "bg-amber-500/10 text-amber-100"
                  : "bg-emerald-500/10 text-emerald-100"
              }`}
            >
              <div className="font-medium">诊断包已生成</div>
              <div className="mt-1 break-all text-xs text-white/60">
                {diagnosticResult.archive_path}
              </div>
              {diagnosticResult.risky_applications.length > 0 && (
                <div className="mt-3">
                  <div className="font-medium text-amber-200">
                    发现可能冲突的应用
                  </div>
                  <ul className="mt-1 space-y-1 text-xs text-amber-100/80">
                    {diagnosticResult.risky_applications.map((app) => (
                      <li key={`${app.pid}-${app.name}`}>
                        {app.name}（{app.process_name}，{app.category}）
                      </li>
                    ))}
                  </ul>
                  <div className="mt-2 text-xs leading-relaxed text-white/70">
                    请关闭这些应用后重试。若仍然崩溃，请在 QQ 群
                    732124612 联系 1259332131，并发送诊断包和复现步骤。
                  </div>
                </div>
              )}
              {diagnosticResult.missing_crash_files.length > 0 && (
                <div className="mt-2 text-xs text-white/60">
                  最新日志目录未找到：
                  {diagnosticResult.missing_crash_files.join("、")}
                  。请在崩溃后重新收集。
                </div>
              )}
              {diagnosticResult.risky_applications.length === 0 && (
                <div className="mt-2 text-xs leading-relaxed text-white/70">
                  未发现明显冲突。请在 QQ 群 732124612 联系
                  1259332131，并发送诊断包和复现步骤。
                </div>
              )}
              <div className="mt-3">
                <PrimaryButton
                  variant="secondary"
                  onClick={() =>
                    openDiagnosticReportFolder(diagnosticResult.archive_path)
                  }
                >
                  打开所在目录
                </PrimaryButton>
              </div>
            </div>
          )}
        </GlassCard>
      </div>

      {/* 自动保存 + 重置 */}
      <div className="sticky bottom-0 -mx-6 px-6 py-3 bg-gradient-to-t from-[#0f1216] to-transparent flex items-center justify-end gap-3">
        {busy === "autosave" && (
          <div className="text-xs text-white/50">保存中…</div>
        )}
        {busy !== "autosave" && savedAt && (
          <div className="text-xs text-emerald-300">已自动保存 ✓</div>
        )}
        {saveError && (
          <div className="text-xs text-red-300">保存失败：{saveError}</div>
        )}
        <PrimaryButton
          variant="secondary"
          size="lg"
          onClick={handleReset}
          disabled={busy === "reset"}
        >
          {busy === "reset" ? "重置中…" : "重置"}
        </PrimaryButton>
      </div>
    </div>
  );
}
