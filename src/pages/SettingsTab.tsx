import { useEffect, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { useSettings } from "../hooks/useSettings";
import { ProxyMode, ProxyTestResult } from "../ipc/types";
import { setProxyMode, testProxy } from "../ipc/proxy";
import { validateInstallPath, openLogFolder } from "../ipc/settings";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

export function SettingsTab() {
  const { settings, loading, error, update } = useSettings();
  const [proxyKind, setProxyKind] = useState<ProxyMode["kind"]>("system");
  const [proxyUrl, setProxyUrl] = useState("");
  const [rootConfigUrl, setRootConfigUrl] = useState("");
  const [libraryRoot, setLibraryRoot] = useState("");
  const [concurrency, setConcurrency] = useState(4);
  const [pathErrors, setPathErrors] = useState<string[]>([]);
  const [pathWarnings, setPathWarnings] = useState<string[]>([]);
  const [proxyResult, setProxyResult] = useState<ProxyTestResult | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [savedAt, setSavedAt] = useState<number | null>(null);

  // Hydrate local form state from settings on first load.
  useEffect(() => {
    if (!settings) return;
    setProxyKind(settings.proxy_mode.kind);
    setProxyUrl(
      settings.proxy_mode.kind === "custom" ? settings.proxy_mode.url : "",
    );
    setRootConfigUrl(settings.root_config_url);
    setLibraryRoot(settings.library_root);
    setConcurrency(settings.concurrent_downloads);
  }, [settings]);

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

  const handleSaveProxy = async () => {
    setBusy("proxy");
    try {
      await setProxyMode(buildProxyMode());
      setSavedAt(Date.now());
    } catch (e) {
      alert(`代理设置失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setBusy(null);
    }
  };

  const handleTestProxy = async () => {
    setBusy("test");
    setProxyResult(null);
    try {
      const r = await testProxy();
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
    }
  };

  const handleSaveGeneral = async () => {
    if (pathErrors.length > 0) return;
    setBusy("general");
    try {
      await update({
        root_config_url: rootConfigUrl.trim(),
        library_root: libraryRoot,
        concurrent_downloads: concurrency,
      });
      setSavedAt(Date.now());
    } catch (e) {
      alert(`保存失败：${e instanceof Error ? e.message : String(e)}`);
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
              variant="primary"
              onClick={handleSaveProxy}
              disabled={busy === "proxy"}
            >
              {busy === "proxy" ? "应用中…" : "应用代理"}
            </PrimaryButton>
            <PrimaryButton
              variant="secondary"
              onClick={handleTestProxy}
              disabled={busy === "test"}
            >
              {busy === "test" ? "测试中…" : "测试连通性"}
            </PrimaryButton>
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
          subtitle="镜像 config.json 的完整 URL（与官方 RemoteConfig 同结构）。"
        />
        <input
          type="url"
          placeholder="https://your-mirror.example.cn/launcher/config.json"
          value={rootConfigUrl}
          onChange={(e) => setRootConfigUrl(e.target.value)}
        />
      </GlassCard>

      {/* 安装位置 */}
      <GlassCard>
        <SectionHeader
          icon="📁"
          title="安装位置"
          subtitle={`实际安装目录：${libraryRoot || "<未选择>"}/R5R Library/<频道>/`}
        />
        <div className="space-y-2">
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

      {/* 保存 */}
      <div className="sticky bottom-0 -mx-6 px-6 py-3 bg-gradient-to-t from-[#0f1216] to-transparent flex items-center justify-end gap-3">
        {savedAt && (
          <div className="text-xs text-emerald-300">已保存 ✓</div>
        )}
        <PrimaryButton
          variant="primary"
          size="lg"
          onClick={handleSaveGeneral}
          disabled={busy === "general" || pathErrors.length > 0}
        >
          {busy === "general" ? "保存中…" : "保存全部"}
        </PrimaryButton>
      </div>
    </div>
  );
}
