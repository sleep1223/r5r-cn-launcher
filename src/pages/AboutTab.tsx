import { useEffect, useRef, useState } from "react";
import { GlassCard, SectionHeader } from "../components/GlassCard";
import { PrimaryButton } from "../components/PrimaryButton";
import { downloadAndApplyUpdate } from "../ipc/updater";
import { listen } from "@tauri-apps/api/event";
import type { LauncherUpdateInfo, UpdateCheckStatus } from "../App";

interface UpdateProgress {
  bytes_done: number;
  bytes_total: number | null;
  phase: string;
}

interface Props {
  pendingUpdate: LauncherUpdateInfo | null;
  onUpdateDismissed: () => void;
  currentVersion: string;
  checkStatus: UpdateCheckStatus;
  checkError: string | null;
  onCheckForUpdate: () => void;
}

function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

export function AboutTab({
  pendingUpdate,
  onUpdateDismissed,
  currentVersion,
  checkStatus,
  checkError,
  onCheckForUpdate,
}: Props) {
  const [updating, setUpdating] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const cancelledRef = useRef(false);

  // Listen to update://progress events from the backend.
  useEffect(() => {
    if (!updating) return;
    const unlisten = listen<UpdateProgress>("update://progress", (e) => {
      setProgress(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [updating]);

  // Auto-start download if force_update is set.
  useEffect(() => {
    if (pendingUpdate?.force && !updating && !updateError) {
      handleStartUpdate();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingUpdate?.force]);

  const handleStartUpdate = async () => {
    if (!pendingUpdate?.url) return;
    cancelledRef.current = false;
    setUpdating(true);
    setUpdateError(null);
    setProgress(null);
    try {
      await downloadAndApplyUpdate(pendingUpdate.url);
    } catch (e) {
      if (!cancelledRef.current) {
        setUpdateError(e instanceof Error ? e.message : String(e));
      }
      setUpdating(false);
      setProgress(null);
    }
  };

  const handleCancel = () => {
    cancelledRef.current = true;
    setUpdating(false);
    setProgress(null);
    setUpdateError(null);
  };

  const pct =
    progress && progress.bytes_total
      ? Math.min(100, (progress.bytes_done / progress.bytes_total) * 100)
      : null;

  return (
    <div className="p-6 max-w-2xl mx-auto space-y-5">
      <GlassCard>
        <div className="text-2xl font-semibold mb-2">R5R 中国镜像启动器</div>
        <div className="text-white/60 mb-4">
          v{currentVersion} · Tauri + React
        </div>
        <div className="text-sm text-white/70 leading-relaxed space-y-2">
          <p>
            本启动器面向中国大陆用户的 Apex（R5Reloaded）社区服。下载协议与官方
            R5Reloaded 启动器完全兼容，但所有 URL 改为可配置的镜像源。
          </p>
          <p className="text-white/50 text-xs">
            非官方项目，仅用于让大陆玩家更顺利地接入社区服务器。
          </p>
        </div>
      </GlassCard>

      {/* Update card */}
      {pendingUpdate ? (
        <GlassCard
          className={
            pendingUpdate.force
              ? "border-red-400/40 bg-red-500/[0.06]"
              : "border-blue-400/40 bg-blue-500/[0.06]"
          }
        >
          <SectionHeader
            title={`新版本 ${pendingUpdate.version} 可用`}
            subtitle={
              pendingUpdate.force
                ? "此版本为强制更新，请尽快完成更新。"
                : "建议更新以获得最新功能和修复。"
            }
          />

          {/* Progress bar */}
          {updating && (
            <div className="space-y-2 mb-3">
              <div className="h-2 rounded-full bg-white/8 overflow-hidden">
                {pct !== null ? (
                  <div
                    className="h-full bg-gradient-to-r from-blue-400 to-emerald-400 transition-all duration-300"
                    style={{ width: `${pct}%` }}
                  />
                ) : (
                  <div className="h-full bg-gradient-to-r from-blue-400 to-emerald-400 animate-pulse" />
                )}
              </div>
              {progress && (
                <div className="text-xs text-white/50 tabular-nums">
                  {formatBytes(progress.bytes_done)}
                  {progress.bytes_total
                    ? ` / ${formatBytes(progress.bytes_total)} (${pct?.toFixed(0)}%)`
                    : ""}
                </div>
              )}
            </div>
          )}

          {updateError && (
            <div className="text-xs px-3 py-2 rounded-lg bg-red-500/10 text-red-300 mb-3">
              更新失败：{updateError}
            </div>
          )}

          <div className="flex items-center gap-2">
            {!updating && (
              <PrimaryButton variant="primary" onClick={handleStartUpdate}>
                {updateError ? "重试更新" : "立即更新"}
              </PrimaryButton>
            )}
            {updating && (
              <PrimaryButton variant="secondary" onClick={handleCancel}>
                取消下载
              </PrimaryButton>
            )}
            {!updating && !pendingUpdate.force && (
              <PrimaryButton variant="secondary" onClick={onUpdateDismissed}>
                稍后
              </PrimaryButton>
            )}
          </div>
        </GlassCard>
      ) : (
        <GlassCard>
          <SectionHeader
            title="检查更新"
            subtitle="启动时已自动检查一次，也可随时手动检查。"
          />
          <div className="flex items-center gap-3 flex-wrap">
            <PrimaryButton
              variant="secondary"
              onClick={onCheckForUpdate}
              disabled={checkStatus === "checking"}
            >
              {checkStatus === "checking" ? "检查中…" : "检查更新"}
            </PrimaryButton>
            {checkStatus === "up-to-date" && (
              <span className="text-sm text-emerald-300">
                ✓ 当前已是最新版本（v{currentVersion}）
              </span>
            )}
            {checkStatus === "idle" && (
              <span className="text-sm text-white/50">
                当前版本 v{currentVersion}
              </span>
            )}
            {checkStatus === "error" && (
              <span className="text-sm text-red-300">
                检查失败{checkError ? `：${checkError}` : ""}
              </span>
            )}
          </div>
        </GlassCard>
      )}
    </div>
  );
}
