import { invoke } from "./invoke";
import { RemoteConfig } from "./types";

export const fetchRemoteConfig = (url?: string) =>
  invoke<RemoteConfig>("fetch_remote_config_cmd", url ? { url } : {});

export const getChannelVersion = (channel: string) =>
  invoke<string>("get_channel_version", { channel });
