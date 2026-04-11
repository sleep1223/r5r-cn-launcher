import { useEffect, useRef, useState } from "react";
import { InstallLogEvent, ProgressEvent } from "../ipc/types";
import { formatBytes, formatEta } from "../hooks/useInstallProgress";
import { PrimaryButton } from "./PrimaryButton";

interface Props {
  progress: ProgressEvent;
  logs?: InstallLogEvent[];
  onCancel?: () => void;
  /** If set, a 暂停/继续 button is shown next to 取消. */
  onTogglePause?: () => void;
  /** Current pause state — drives the toggle label. */
  paused?: boolean;
}

const PHASE_LABELS: Record<string, string> = {
  preparing: "准备中",
  fetching_config: "拉取镜像 config 中",
  fetching_manifest: "拉取游戏 manifest 中",
  scanning: "校验已下载文件中",
  downloading: "下载/复制中",
  merging_parts: "合并分块中",
  verifying: "最终校验中",
  complete: "完成",
  failed: "失败",
  cancelled: "已取消",
};

export function InstallProgress({
  progress,
  logs,
  onCancel,
  onTogglePause,
  paused = false,
}: Props) {
  const phase = progress.phase.phase;
  const [showLogs, setShowLogs] = useState(false);
  const logScrollRef = useRef<HTMLDivElement | null>(null);

  // Auto-scroll the log panel to the latest line as new logs arrive — only
  // when the panel is open, otherwise we'd burn cycles for nothing.
  useEffect(() => {
    if (!showLogs) return;
    const el = logScrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [showLogs, logs]);

  const pct =
    progress.bytes_total > 0
      ? Math.min(100, (progress.bytes_done / progress.bytes_total) * 100)
      : phase === "scanning" && progress.file_count > 0
        ? Math.min(100, (progress.file_index / progress.file_count) * 100)
        : 0;

  const isFinal = phase === "complete" || phase === "failed" || phase === "cancelled";

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3 flex-wrap">
        <span className="text-sm font-medium">
          {paused ? "已暂停" : (PHASE_LABELS[phase] ?? phase)}
        </span>
        {progress.file_count > 0 && (
          <span className="text-xs text-white/40">
            {progress.file_index} / {progress.file_count} 文件
          </span>
        )}
        {!isFinal && phase === "downloading" && progress.speed_bps > 0 && (
          <span className="text-xs text-white/40">
            {formatBytes(progress.speed_bps)}/s · 剩余 {formatEta(progress.eta_seconds)}
          </span>
        )}
        {logs && logs.length > 0 && (
          <button
            type="button"
            onClick={() => setShowLogs((v) => !v)}
            className="ml-auto text-xs text-white/60 hover:text-white underline-offset-2 hover:underline"
          >
            {showLogs ? "隐藏日志" : `查看日志（${logs.length}）`}
          </button>
        )}
      </div>

      <div className="h-2 rounded-full bg-white/8 overflow-hidden">
        <div
          className="h-full bg-gradient-to-r from-blue-400 to-emerald-400 transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>

      <div className="flex items-center justify-between text-xs text-white/50">
        <span className="truncate min-w-0 flex-1 font-mono">
          {progress.current_file || "\u00a0"}
        </span>
        {progress.bytes_total > 0 && (
          <span className="ml-3 tabular-nums">
            {formatBytes(progress.bytes_done)} / {formatBytes(progress.bytes_total)}
          </span>
        )}
      </div>

      {phase === "failed" && "reason" in progress.phase && (
        <div className="text-xs text-red-300 px-3 py-2 rounded-lg bg-red-500/10">
          {(progress.phase as { reason: string }).reason}
        </div>
      )}

      {showLogs && logs && (
        <div
          ref={logScrollRef}
          className="text-[11px] font-mono leading-relaxed bg-black/30 rounded-lg px-3 py-2 max-h-48 overflow-y-auto border border-white/5"
        >
          {logs.length === 0 ? (
            <div className="text-white/40">暂无日志</div>
          ) : (
            logs.map((line, i) => (
              <div
                key={`${line.ts_ms}-${i}`}
                className={
                  line.level === "error"
                    ? "text-red-300"
                    : line.level === "warn"
                      ? "text-amber-300"
                      : "text-white/70"
                }
              >
                <span className="text-white/30 mr-2">
                  {formatTs(line.ts_ms)}
                </span>
                {line.message}
              </div>
            ))
          )}
        </div>
      )}

      {!isFinal && (onCancel || onTogglePause) && (
        <div className="flex items-center gap-2">
          {onTogglePause && (
            <PrimaryButton variant="secondary" onClick={onTogglePause}>
              {paused ? "继续" : "暂停"}
            </PrimaryButton>
          )}
          {onCancel && (
            <PrimaryButton variant="secondary" onClick={onCancel}>
              取消
            </PrimaryButton>
          )}
        </div>
      )}
    </div>
  );
}

function formatTs(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}
