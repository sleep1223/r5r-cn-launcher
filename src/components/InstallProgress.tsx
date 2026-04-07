import { ProgressEvent } from "../ipc/types";
import { formatBytes, formatEta } from "../hooks/useInstallProgress";
import { PrimaryButton } from "./PrimaryButton";

interface Props {
  progress: ProgressEvent;
  onCancel?: () => void;
}

export function InstallProgress({ progress, onCancel }: Props) {
  const phase = progress.phase.phase;
  const pct =
    progress.bytes_total > 0
      ? Math.min(100, (progress.bytes_done / progress.bytes_total) * 100)
      : 0;

  const phaseLabel: Record<string, string> = {
    preparing: "准备中",
    downloading: "下载/复制中",
    merging_parts: "合并分块",
    verifying: "校验",
    complete: "完成",
    failed: "失败",
    cancelled: "已取消",
  };

  const isFinal = phase === "complete" || phase === "failed" || phase === "cancelled";

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3">
        <span className="text-sm font-medium">{phaseLabel[phase] ?? phase}</span>
        {progress.file_count > 0 && (
          <span className="text-xs text-white/40">
            {progress.file_index} / {progress.file_count} 文件
          </span>
        )}
        {!isFinal && progress.speed_bps > 0 && (
          <span className="text-xs text-white/40">
            {formatBytes(progress.speed_bps)}/s · 剩余 {formatEta(progress.eta_seconds)}
          </span>
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
        <span className="ml-3 tabular-nums">
          {formatBytes(progress.bytes_done)} / {formatBytes(progress.bytes_total)}
        </span>
      </div>

      {phase === "failed" && "reason" in progress.phase && (
        <div className="text-xs text-red-300 px-3 py-2 rounded-lg bg-red-500/10">
          {(progress.phase as { reason: string }).reason}
        </div>
      )}

      {!isFinal && onCancel && (
        <div>
          <PrimaryButton variant="secondary" onClick={onCancel}>
            取消
          </PrimaryButton>
        </div>
      )}
    </div>
  );
}
