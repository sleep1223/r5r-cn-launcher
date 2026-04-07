import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { AppError, AppErrorPayload } from "./types";

/** Wrapper that converts the Rust `AppError` payload into a JS `AppError`. */
export async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return (await tauriInvoke(cmd, args)) as T;
  } catch (raw) {
    if (raw && typeof raw === "object" && "kind" in raw && "message" in raw) {
      throw new AppError(raw as AppErrorPayload);
    }
    if (typeof raw === "string") {
      throw new AppError({ kind: "other", message: raw });
    }
    throw new AppError({
      kind: "other",
      message: raw instanceof Error ? raw.message : String(raw),
    });
  }
}
