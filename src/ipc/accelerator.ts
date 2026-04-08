import { invoke } from "./invoke";

export interface DetectedAccelerator {
  name: string;
  process_name: string;
  pid: number;
}

export const detectAccelerators = () =>
  invoke<DetectedAccelerator[]>("detect_accelerators_cmd");
