import { useState } from "react";
import { Sidebar, TabId } from "./components/Sidebar";
import { HomeTab } from "./pages/HomeTab";
import { ServersTab } from "./pages/ServersTab";
import { LeaderboardTab } from "./pages/LeaderboardTab";
import { TeamsTab } from "./pages/TeamsTab";
import { ProfileTab } from "./pages/ProfileTab";
import { LaunchOptionsTab } from "./pages/LaunchOptionsTab";
import { SettingsTab } from "./pages/SettingsTab";
import { AboutTab } from "./pages/AboutTab";
import { SettingsProvider } from "./hooks/useSettings";
import { AuthProvider } from "./hooks/useAuth";

export interface LauncherUpdateInfo {
  version: string;
  url: string;
  force: boolean;
}

function App() {
  const [tab, setTab] = useState<TabId>("home");
  const [pendingUpdate, setPendingUpdate] = useState<LauncherUpdateInfo | null>(
    null,
  );

  const handleUpdateDetected = (info: LauncherUpdateInfo) => {
    setPendingUpdate(info);
    setTab("about");
  };

  return (
    <SettingsProvider>
      <AuthProvider>
        <div className="h-screen w-screen flex overflow-hidden">
          <Sidebar active={tab} onChange={setTab} />
          <main className="flex-1 overflow-y-auto">
            {tab === "home" && (
              <HomeTab onUpdateDetected={handleUpdateDetected} />
            )}
            {tab === "servers" && <ServersTab />}
            {tab === "leaderboard" && <LeaderboardTab />}
            {tab === "teams" && <TeamsTab />}
            {tab === "profile" && <ProfileTab />}
            {tab === "launch_options" && <LaunchOptionsTab />}
            {tab === "settings" && <SettingsTab />}
            {tab === "about" && (
              <AboutTab
                pendingUpdate={pendingUpdate}
                onUpdateDismissed={() => setPendingUpdate(null)}
              />
            )}
          </main>
        </div>
      </AuthProvider>
    </SettingsProvider>
  );
}

export default App;
