import { invoke } from "./invoke";
import { ProxyMode, ProxyTestResult } from "./types";

export const setProxyMode = (mode: ProxyMode) =>
  invoke<void>("set_proxy_mode", { mode });

/// Pass `overrideDomain` to test a mirror domain the user is currently editing
/// without requiring them to save first. Empty/undefined falls back to the
/// saved mirror domain, and from there to the official CDN.
export const testProxy = (overrideDomain?: string) =>
  invoke<ProxyTestResult>("test_proxy", { overrideDomain: overrideDomain ?? null });
