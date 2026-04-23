import { invoke } from "./invoke";

export interface PingResult {
  ok: boolean;
  latency_ms: number | null;
  error: string | null;
}

export const pingHost = (host: string, port: number, timeoutMs = 2000) =>
  invoke<PingResult>("ping_host", { host, port, timeoutMs });
