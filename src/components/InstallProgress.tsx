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
  fetching_manifest: "拉取游戏 manifest 中",
  scanning: "校验已下载文件中",
  downloading: "下载 / 解压中",
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

  useEffect(() => {
    if (!showLogs) return;
    const el = logScrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [showLogs, logs]);

  // Progress percent: prefer byte ratio (most accurate), fall back to file
  // ratio during the prep scan where bytes_total is still 0.
  const pct =
    progress.bytes_total > 0
      ? Math.min(100, (progress.bytes_done / progress.bytes_total) * 100)
      : progress.file_count > 0
        ? Math.min(100, (progress.file_index / progress.file_count) * 100)
        : 0;

  const isFinal =
    phase === "complete" || phase === "failed" || phase === "cancelled";
  const phaseLabel = paused ? "已暂停" : (PHASE_LABELS[phase] ?? phase);
  const barColor =
    phase === "failed"
      ? "from-red-400 to-red-500"
      : phase === "cancelled"
        ? "from-white/30 to-white/40"
        : phase === "complete"
          ? "from-emerald-400 to-emerald-500"
          : "from-blue-400 to-emerald-400";

  const hasBytes = progress.bytes_total > 0;
  const hasFiles = progress.file_count > 0;
  const hasSpeed = !isFinal && progress.speed_bps > 0;
  const hasEta = !isFinal && progress.eta_seconds > 0 && hasBytes;

  return (
    <div className="space-y-3">
      {/* Top row: phase label + percentage */}
      <div className="flex items-baseline justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm font-semibold text-white">{phaseLabel}</span>
          {hasFiles && (
            <span className="text-xs text-white/50 tabular-nums">
              {progress.file_index.toLocaleString()} /{" "}
              {progress.file_count.toLocaleString()} 文件
            </span>
          )}
        </div>
        <span className="text-xl font-bold tabular-nums text-white">
          {pct.toFixed(1)}
          <span className="text-sm font-normal text-white/50 ml-0.5">%</span>
        </span>
      </div>

      {/* Big progress bar */}
      <div className="h-2.5 rounded-full bg-white/8 overflow-hidden">
        <div
          className={`h-full bg-gradient-to-r ${barColor} transition-all`}
          style={{ width: `${pct}%` }}
        />
      </div>

      {/* Current file */}
      <div
        className="text-xs font-mono text-white/60 truncate"
        title={progress.current_file}
      >
        {progress.current_file || "\u00a0"}
      </div>

      {/* Stats grid: 已处理 / 总计 / 速度 / 剩余时间 */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
        <Stat
          label="已处理"
          value={
            hasBytes
              ? formatBytes(progress.bytes_done)
              : hasFiles
                ? `${progress.file_index.toLocaleString()} 个`
                : "—"
          }
        />
        <Stat
          label="总计"
          value={
            hasBytes
              ? formatBytes(progress.bytes_total)
              : hasFiles
                ? `${progress.file_count.toLocaleString()} 个`
                : "—"
          }
        />
        <Stat
          label="速度"
          value={hasSpeed ? `${formatBytes(progress.speed_bps)}/s` : "—"}
        />
        <Stat label="剩余" value={hasEta ? formatEta(progress.eta_seconds) : "—"} />
      </div>

      {/* Failure reason, if any */}
      {phase === "failed" && "reason" in progress.phase && (
        <div className="text-xs text-red-300 px-3 py-2 rounded-lg bg-red-500/10">
          {(progress.phase as { reason: string }).reason}
        </div>
      )}

      {/* Logs */}
      {logs && logs.length > 0 && (
        <div className="flex items-center">
          <button
            type="button"
            onClick={() => setShowLogs((v) => !v)}
            className="text-xs text-white/60 hover:text-white underline-offset-2 hover:underline"
          >
            {showLogs ? "隐藏日志" : `查看日志（${logs.length}）`}
          </button>
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
                <span className="text-white/30 mr-2">{formatTs(line.ts_ms)}</span>
                {line.message}
              </div>
            ))
          )}
        </div>
      )}

      {/* Cancel / pause */}
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

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="px-3 py-2 rounded-lg bg-white/[0.03] border border-white/5">
      <div className="text-[10px] text-white/40 uppercase tracking-wider">
        {label}
      </div>
      <div className="text-xs font-mono text-white/85 tabular-nums mt-0.5 truncate">
        {value}
      </div>
    </div>
  );
}

function formatTs(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}
