import { invoke } from "./invoke";
import { DashboardConfig } from "./types";

export const fetchDashboardConfig = (url?: string) =>
  invoke<DashboardConfig>("fetch_dashboard_config_cmd", url ? { url } : {});
