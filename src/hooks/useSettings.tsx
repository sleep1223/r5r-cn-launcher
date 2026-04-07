import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { LauncherSettings } from "../ipc/types";
import { loadSettings, saveSettings } from "../ipc/settings";

interface Ctx {
  settings: LauncherSettings | null;
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
  update: (patch: Partial<LauncherSettings>) => Promise<void>;
}

const SettingsContext = createContext<Ctx | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<LauncherSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const s = await loadSettings();
      setSettings(s);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const update = useCallback(
    async (patch: Partial<LauncherSettings>) => {
      if (!settings) return;
      const next = { ...settings, ...patch };
      await saveSettings(next);
      setSettings(next);
    },
    [settings],
  );

  return (
    <SettingsContext.Provider value={{ settings, loading, error, reload, update }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) throw new Error("useSettings must be used inside SettingsProvider");
  return ctx;
}
