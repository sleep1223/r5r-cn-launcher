import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  DetectedAccelerator,
  detectAccelerators,
} from "../ipc/accelerator";

/**
 * Subscribes to the backend's accelerator scanner. Performs an initial
 * synchronous detect on mount and then listens on `accelerator://changed`
 * for live updates as the user starts/stops their VPN/accelerator process.
 */
export function useAccelerators(): DetectedAccelerator[] {
  const [list, setList] = useState<DetectedAccelerator[]>([]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    (async () => {
      try {
        const initial = await detectAccelerators();
        if (!cancelled) setList(initial);
      } catch {
        /* swallow — detection is best-effort */
      }
    })();

    (async () => {
      unlisten = await listen<DetectedAccelerator[]>(
        "accelerator://changed",
        (e) => {
          if (!cancelled) setList(e.payload);
        },
      );
    })();

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  return list;
}
