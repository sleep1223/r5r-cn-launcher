import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { InstallLogEvent, ProgressEvent } from "../ipc/types";

/// Subscribes to `install://progress` and exposes the latest snapshot.
/// `null` until the first event arrives.
export function useInstallProgress(): ProgressEvent | null {
  const [progress, setProgress] = useState<ProgressEvent | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await listen<ProgressEvent>("install://progress", (e) => {
        setProgress(e.payload);
      });
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return progress;
}

const MAX_LOG_LINES = 500;

/**
 * Subscribes to `install://log` and accumulates lines for the given job. Lines
 * for other jobs are ignored. Pass `null` to clear the buffer.
 */
export function useInstallLog(jobId: string | null): InstallLogEvent[] {
  const [logs, setLogs] = useState<InstallLogEvent[]>([]);

  // Reset whenever the active job changes (including back to null).
  useEffect(() => {
    setLogs([]);
  }, [jobId]);

  useEffect(() => {
    if (!jobId) return;
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await listen<InstallLogEvent>("install://log", (e) => {
        if (e.payload.job_id !== jobId) return;
        setLogs((prev) => {
          const next = prev.length >= MAX_LOG_LINES ? prev.slice(1) : prev.slice();
          next.push(e.payload);
          return next;
        });
      });
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [jobId]);

  return logs;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

export function formatEta(seconds: number): string {
  if (seconds <= 0) return "—";
  if (seconds < 60) return `${seconds}s`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  if (m < 60) return `${m}m ${s}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}
