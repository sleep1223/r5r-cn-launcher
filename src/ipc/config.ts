import { invoke } from "./invoke";

export const getChannelVersion = (channel: string) =>
  invoke<string>("get_channel_version", { channel });
