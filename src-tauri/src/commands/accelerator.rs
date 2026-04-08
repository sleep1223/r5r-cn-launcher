use crate::accelerator::{detect, DetectedAccelerator};

#[tauri::command]
pub fn detect_accelerators_cmd() -> Vec<DetectedAccelerator> {
    detect()
}
