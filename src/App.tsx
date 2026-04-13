import { useState } from "react";
import { Sidebar, TabId } from "./components/Sidebar";
import { HomeTab } from "./pages/HomeTab";
import { LaunchOptionsTab } from "./pages/LaunchOptionsTab";
import { SettingsTab } from "./pages/SettingsTab";
import { AboutTab } from "./pages/AboutTab";
import { SettingsProvider } from "./hooks/useSettings";

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
      <div className="h-screen w-screen flex overflow-hidden">
        <Sidebar active={tab} onChange={setTab} />
        <main className="flex-1 overflow-y-auto">
          {tab === "home" && (
            <HomeTab onUpdateDetected={handleUpdateDetected} />
          )}
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
    </SettingsProvider>
  );
}

export default App;
