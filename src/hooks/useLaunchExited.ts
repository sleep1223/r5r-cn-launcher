import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { LaunchExitedEvent } from "../ipc/types";

export function useLaunchExited(): LaunchExitedEvent | null {
  const [last, setLast] = useState<LaunchExitedEvent | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      unlisten = await listen<LaunchExitedEvent>("launch://exited", (e) => {
        setLast(e.payload);
      });
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return last;
}
