import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ProgressEvent } from "../ipc/types";

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
