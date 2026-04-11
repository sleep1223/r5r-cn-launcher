import { useState } from "react";
import { Sidebar, TabId } from "./components/Sidebar";
import { HomeTab } from "./pages/HomeTab";
import { LaunchOptionsTab } from "./pages/LaunchOptionsTab";
import { SettingsTab } from "./pages/SettingsTab";
import { AboutTab } from "./pages/AboutTab";
import { SettingsProvider } from "./hooks/useSettings";

function App() {
  const [tab, setTab] = useState<TabId>("home");

  return (
    <SettingsProvider>
      <div className="h-screen w-screen flex overflow-hidden">
        <Sidebar active={tab} onChange={setTab} />
        <main className="flex-1 overflow-y-auto">
          {tab === "home" && <HomeTab onNavigate={setTab} />}
          {tab === "launch_options" && <LaunchOptionsTab />}
          {tab === "settings" && <SettingsTab />}
          {tab === "about" && <AboutTab />}
        </main>
      </div>
    </SettingsProvider>
  );
}

export default App;
