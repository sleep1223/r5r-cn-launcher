import { useEffect, useMemo, useRef, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { useSettings } from "../hooks/useSettings";
import { DetectedInstall, ProxyMode, ProxyTestResult } from "../ipc/types";
import { setProxyMode, testProxy } from "../ipc/proxy";
import { validateInstallPath, openLogFolder } from "../ipc/settings";
import { detectExistingR5R } from "../ipc/detect";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

const CUSTOM_OPTION = "__custom__";

/**
 * Walk up `detectedPath` looking for the `R5R Library` segment and return its
 * parent — that's what we need to put into `settings.library_root`. The
 * detected path itself is the *channel* dir (e.g. `C:\R5R Library\LIVE`).
 */
function detectedToLibraryRoot(detectedPath: string): string {
  const segs = detectedPath.split(/[\\/]/);
  for (let i = segs.length - 1; i > 0; i--) {
    if (segs[i].toLowerCase() === "r5r library") {
      const parent = segs.slice(0, i).join("\\");
      // `C:` alone means "current dir on C drive"; we want the actual root.
      return /^[a-z]:$/i.test(parent) ? parent + "\\" : parent;
    }
  }
  // Detected path doesn't include `R5R Library` (e.g. shortcut points
  // somewhere odd) — fall back to using it directly.
  return detectedPath;
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

export function SettingsTab() {
  const { settings, loading, error, update, reload } = useSettings();
  const [proxyKind, setProxyKind] = useState<ProxyMode["kind"]>("system");
  const [proxyUrl, setProxyUrl] = useState("");
  const [rootConfigUrl, setRootConfigUrl] = useState("");
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
    setRootConfigUrl(settings.root_config_url);
    setLibraryRoot(settings.library_root);
    setConcurrency(settings.concurrent_downloads);
    hydrated.current = true;
  }, [settings]);

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
    const match = detectedRoots.find(
      (r) => r.libraryRoot.toLowerCase() === libraryRoot.toLowerCase(),
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
      // Use whatever the user is currently typing in the mirror field — they
      // shouldn't have to hit Save just to verify a URL works. The backend
      // falls back to the official R5R URL if both override and saved are
      // empty, which lets users sanity-check their proxy out of the box.
      const r = await testProxy(rootConfigUrl.trim() || undefined);
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
    const trimmedConfigUrl = rootConfigUrl.trim();

    const proxyChanged =
      JSON.stringify(nextProxy) !== JSON.stringify(settings.proxy_mode);
    const configUrlChanged = trimmedConfigUrl !== settings.root_config_url;
    const libraryRootChanged = libraryRoot !== settings.library_root;
    const concurrencyChanged = concurrency !== settings.concurrent_downloads;

    if (
      !proxyChanged &&
      !configUrlChanged &&
      !libraryRootChanged &&
      !concurrencyChanged
    ) {
      return;
    }

    const handle = window.setTimeout(async () => {
      setBusy("autosave");
      setSaveError(null);
      try {
        // Apply proxy first so a failed rebuild surfaces before we touch the
        // rest of the settings file.
        if (proxyChanged) {
          await setProxyMode(nextProxy);
        }
        await update({
          proxy_mode: nextProxy,
          root_config_url: trimmedConfigUrl,
          library_root: libraryRoot,
          concurrent_downloads: concurrency,
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
  }, [proxyKind, proxyUrl, rootConfigUrl, libraryRoot, concurrency, pathErrors.length]);

  const handleReset = async () => {
    setBusy("reset");
    setSaveError(null);
    try {
      await reload();
      setSavedAt(null);
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
          subtitle="代理切换会立即重建 HTTP 客户端，但已经在下载中的文件不会被打断。"
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
              未填写镜像源时会用官方 URL 进行测试。
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
          subtitle="镜像 config.json 的完整 URL（与官方 RemoteConfig 同结构）。修改后会自动保存，无需手动保存。"
        />
        <input
          type="url"
          placeholder="https://cdn.r5r.org/launcher/config.json"
          value={rootConfigUrl}
          onChange={(e) => setRootConfigUrl(e.target.value)}
        />
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
          subtitle="同时下载多少个文件。每个分块文件内部最多 8 个并发分块。建议 4-8。"
        />
        <div className="flex items-center gap-4">
          <input
            type="range"
            min={1}
            max={16}
            value={concurrency}
            onChange={(e) => setConcurrency(Number(e.target.value))}
            className="flex-1"
          />
          <div className="w-10 text-right tabular-nums">{concurrency}</div>
        </div>
      </GlassCard>

      {/* 高级 */}
      <GlassCard>
        <SectionHeader icon="⚒" title="高级" />
        <div className="flex gap-2">
          <PrimaryButton variant="secondary" onClick={() => openLogFolder()}>
            打开日志目录
          </PrimaryButton>
        </div>
      </GlassCard>

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
