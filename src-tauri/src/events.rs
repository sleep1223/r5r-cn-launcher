//! Tauri event payloads emitted from background tasks.

use serde::Serialize;
use uuid::Uuid;

pub const EVT_INSTALL_PROGRESS: &str = "install://progress";
pub const EVT_LAUNCH_EXITED: &str = "launch://exited";
pub const EVT_PROXY_CHANGED: &str = "proxy://changed";

pub type InstallJobId = String;

pub fn new_job_id() -> InstallJobId {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum InstallPhase {
    Preparing,
    Downloading,
    MergingParts,
    Verifying,
    Complete,
    Failed { reason: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub job_id: InstallJobId,
    pub phase: InstallPhase,
    pub file_index: usize,
    pub file_count: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current_file: String,
    pub speed_bps: u64,
    pub eta_seconds: u64,
}

impl ProgressEvent {
    pub fn empty(job_id: InstallJobId, phase: InstallPhase) -> Self {
        Self {
            job_id,
            phase,
            file_index: 0,
            file_count: 0,
            bytes_done: 0,
            bytes_total: 0,
            current_file: String::new(),
            speed_bps: 0,
            eta_seconds: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchExitedEvent {
    pub pid: u32,
    pub code: Option<i32>,
    pub success: bool,
}
