import { invoke } from "./invoke";
import { DetectedInstall } from "./types";

export const detectExistingR5R = () =>
  invoke<DetectedInstall[]>("detect_existing_r5r");
