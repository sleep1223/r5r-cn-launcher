import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { getUserMe, UserInfo } from "../api";

interface AuthCtx {
  appKey: string | null;
  user: UserInfo | null;
  loading: boolean;
  error: string | null;
  login: (key: string) => Promise<boolean>;
  logout: () => void;
}

const Ctx = createContext<AuthCtx | null>(null);

const STORAGE_KEY = "r5_app_key";
const STORAGE_USER = "r5_user_info";

export function AuthProvider({ children }: { children: ReactNode }) {
  const [appKey, setAppKey] = useState<string | null>(
    () => localStorage.getItem(STORAGE_KEY),
  );
  const [user, setUser] = useState<UserInfo | null>(() => {
    try {
      const s = localStorage.getItem(STORAGE_USER);
      return s ? JSON.parse(s) : null;
    } catch {
      return null;
    }
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const login = useCallback(async (key: string): Promise<boolean> => {
    setLoading(true);
    setError(null);
    try {
      const info = await getUserMe(key);
      setAppKey(key);
      setUser(info);
      localStorage.setItem(STORAGE_KEY, key);
      localStorage.setItem(STORAGE_USER, JSON.stringify(info));
      return true;
    } catch {
      setError("AppKey 无效或网络错误");
      return false;
    } finally {
      setLoading(false);
    }
  }, []);

  const logout = useCallback(() => {
    setAppKey(null);
    setUser(null);
    localStorage.removeItem(STORAGE_KEY);
    localStorage.removeItem(STORAGE_USER);
  }, []);

  // Validate stored session on mount.
  useEffect(() => {
    if (!appKey) return;
    let cancelled = false;
    (async () => {
      try {
        const info = await getUserMe(appKey);
        if (!cancelled) {
          setUser(info);
          localStorage.setItem(STORAGE_USER, JSON.stringify(info));
        }
      } catch {
        if (!cancelled) {
          // Stored key is stale — clear it silently.
          setAppKey(null);
          setUser(null);
          localStorage.removeItem(STORAGE_KEY);
          localStorage.removeItem(STORAGE_USER);
        }
      }
    })();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Ctx.Provider value={{ appKey, user, loading, error, login, logout }}>
      {children}
    </Ctx.Provider>
  );
}

export function useAuth() {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useAuth must be inside AuthProvider");
  return ctx;
}
