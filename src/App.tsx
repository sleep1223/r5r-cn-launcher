import { useCallback, useEffect, useRef, useState } from "react";
import { Sidebar, TabId } from "./components/Sidebar";
import { HomeTab } from "./pages/HomeTab";
import { ServersTab } from "./pages/ServersTab";
import { LeaderboardTab } from "./pages/LeaderboardTab";
import { ApexTab } from "./pages/ApexTab";
import { TeamsTab } from "./pages/TeamsTab";
import { ProfileTab } from "./pages/ProfileTab";
import { LaunchOptionsTab } from "./pages/LaunchOptionsTab";
import { SettingsTab } from "./pages/SettingsTab";
import { AboutTab } from "./pages/AboutTab";
import { ConfigSyncTab } from "./pages/ConfigSyncTab";
import { SettingsProvider } from "./hooks/useSettings";
import { AuthProvider } from "./hooks/useAuth";
import { fetchDashboardConfig } from "./ipc/dashboard";
import { getLauncherVersion } from "./ipc/updater";

export interface LauncherUpdateInfo {
  version: string;
  url: string;
  force: boolean;
}

export type UpdateCheckStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "error";

function App() {
  const [tab, setTab] = useState<TabId>("home");
  const [focusDiagnostics, setFocusDiagnostics] = useState(false);
  const [pendingUpdate, setPendingUpdate] = useState<LauncherUpdateInfo | null>(
    null,
  );
  const [currentVersion, setCurrentVersion] = useState<string>(__APP_VERSION__);
  const [checkStatus, setCheckStatus] = useState<UpdateCheckStatus>("idle");
  const [checkError, setCheckError] = useState<string | null>(null);
  // Guards the one-shot boot-time auto check so StrictMode double-invokes and
  // tab re-mounts can't trigger a second check.
  const bootCheckFiredRef = useRef(false);

  const runUpdateCheck = useCallback(
    async (opts?: { navigateOnFound?: boolean }): Promise<void> => {
      setCheckStatus("checking");
      setCheckError(null);
      try {
        const [v, d] = await Promise.all([
          getLauncherVersion(),
          fetchDashboardConfig(),
        ]);
        setCurrentVersion(v.current);
        if (!d.launcher_version || !d.launcher_update_url) {
          setPendingUpdate(null);
          setCheckStatus("up-to-date");
          return;
        }
        const parse = (s: string) => s.replace(/^v/, "").split(".").map(Number);
        const c = parse(v.current);
        const r = parse(d.launcher_version);
        const isNewer =
          r[0] > c[0] ||
          (r[0] === c[0] && r[1] > c[1]) ||
          (r[0] === c[0] && r[1] === c[1] && r[2] > c[2]);
        if (isNewer) {
          setPendingUpdate({
            version: d.launcher_version,
            url: d.launcher_update_url,
            force: d.force_update,
          });
          setCheckStatus("available");
          if (opts?.navigateOnFound) setTab("about");
        } else {
          setPendingUpdate(null);
          setCheckStatus("up-to-date");
        }
      } catch (e) {
        // eslint-disable-next-line no-console
        console.warn("update check failed:", e);
        setCheckStatus("error");
        setCheckError(e instanceof Error ? e.message : String(e));
      }
    },
    [],
  );

  // Fire exactly once per launcher run — never on tab switch / re-mount.
  useEffect(() => {
    if (bootCheckFiredRef.current) return;
    bootCheckFiredRef.current = true;
    runUpdateCheck({ navigateOnFound: true });
  }, [runUpdateCheck]);

  return (
    <SettingsProvider>
      <AuthProvider>
        <div className="h-screen w-screen flex overflow-hidden">
          <Sidebar
            active={tab}
            onChange={(nextTab) => {
              setFocusDiagnostics(false);
              setTab(nextTab);
            }}
          />
          <main className="flex-1 overflow-y-auto">
            {tab === "home" && (
              <HomeTab
                onOpenDiagnostics={() => {
                  setFocusDiagnostics(true);
                  setTab("settings");
                }}
              />
            )}
            {tab === "servers" && <ServersTab />}
            {tab === "leaderboard" && <LeaderboardTab />}
            {tab === "apex" && <ApexTab />}
            {tab === "teams" && <TeamsTab />}
            {tab === "profile" && <ProfileTab />}
            {tab === "launch_options" && <LaunchOptionsTab />}
            {tab === "config_sync" && <ConfigSyncTab />}
            {tab === "settings" && (
              <SettingsTab focusDiagnostics={focusDiagnostics} />
            )}
            {tab === "about" && (
              <AboutTab
                pendingUpdate={pendingUpdate}
                onUpdateDismissed={() => setPendingUpdate(null)}
                currentVersion={currentVersion}
                checkStatus={checkStatus}
                checkError={checkError}
                onCheckForUpdate={() =>
                  runUpdateCheck({ navigateOnFound: false })
                }
              />
            )}
          </main>
        </div>
      </AuthProvider>
    </SettingsProvider>
  );
}

export default App;
