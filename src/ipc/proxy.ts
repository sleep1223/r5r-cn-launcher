import { invoke } from "./invoke";
import { ProxyMode, ProxyTestResult } from "./types";

export const setProxyMode = (mode: ProxyMode) =>
  invoke<void>("set_proxy_mode", { mode });

/// Pass `overrideUrl` to test a URL the user is currently editing without
/// requiring them to save first. Empty/undefined falls back to the saved
/// mirror URL, and from there to the official R5R config URL.
export const testProxy = (overrideUrl?: string) =>
  invoke<ProxyTestResult>("test_proxy", { overrideUrl: overrideUrl ?? null });
