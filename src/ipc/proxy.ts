import { invoke } from "./invoke";
import { ProxyMode, ProxyTestResult } from "./types";

export const setProxyMode = (mode: ProxyMode) =>
  invoke<void>("set_proxy_mode", { mode });

export const testProxy = () => invoke<ProxyTestResult>("test_proxy");
