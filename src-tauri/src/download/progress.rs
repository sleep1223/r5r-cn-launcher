use crate::events::{InstallPhase, ProgressEvent, EVT_INSTALL_PROGRESS};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

/// Aggregates per-byte progress from many download workers and emits a
/// snapshot to the frontend every 200ms.
pub struct ProgressAggregator {
    pub job_id: String,
    pub file_count: usize,
    pub total_bytes: u64,
    bytes_done: AtomicU64,
    files_done: AtomicUsize,
    current_file: Mutex<String>,
    samples: Mutex<VecDeque<(Instant, u64)>>,
    started: Instant,
}

impl ProgressAggregator {
    pub fn new(job_id: String, file_count: usize, total_bytes: u64) -> Arc<Self> {
        Arc::new(Self {
            job_id,
            file_count,
            total_bytes,
            bytes_done: AtomicU64::new(0),
            files_done: AtomicUsize::new(0),
            current_file: Mutex::new(String::new()),
            samples: Mutex::new(VecDeque::new()),
            started: Instant::now(),
        })
    }

    pub fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
        let mut s = self.samples.lock();
        let now = Instant::now();
        s.push_back((now, n));
        // Drop samples older than 500ms.
        while let Some((t, _)) = s.front() {
            if now.duration_since(*t) > Duration::from_millis(500) {
                s.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn finish_file(&self, name: &str) {
        self.files_done.fetch_add(1, Ordering::Relaxed);
        *self.current_file.lock() = name.to_string();
    }

    pub fn set_current_file(&self, name: &str) {
        *self.current_file.lock() = name.to_string();
    }

    pub fn snapshot(&self, phase: InstallPhase) -> ProgressEvent {
        let bytes_done = self.bytes_done.load(Ordering::Relaxed);
        let files_done = self.files_done.load(Ordering::Relaxed);
        let s = self.samples.lock();
        let window_bytes: u64 = s.iter().map(|(_, b)| b).sum();
        let window_dur = s
            .front()
            .map(|(t, _)| Instant::now().duration_since(*t))
            .unwrap_or(Duration::from_millis(1));
        let speed_bps = if window_dur.as_secs_f64() > 0.0 {
            (window_bytes as f64 / window_dur.as_secs_f64()) as u64
        } else {
            0
        };
        let eta_seconds = if speed_bps > 0 {
            self.total_bytes.saturating_sub(bytes_done) / speed_bps
        } else {
            0
        };
        ProgressEvent {
            job_id: self.job_id.clone(),
            phase,
            file_index: files_done,
            file_count: self.file_count,
            bytes_done,
            bytes_total: self.total_bytes,
            current_file: self.current_file.lock().clone(),
            speed_bps,
            eta_seconds,
        }
    }

    /// Spawn a background task that emits a snapshot every 200ms until
    /// cancelled.
    pub fn spawn_emitter(
        self: &Arc<Self>,
        app: AppHandle,
        cancel: CancellationToken,
        phase: InstallPhase,
    ) -> tauri::async_runtime::JoinHandle<()> {
        let agg = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut t = tokio::time::interval(Duration::from_millis(200));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = t.tick() => {
                        let _ = app.emit(EVT_INSTALL_PROGRESS, agg.snapshot(phase.clone()));
                    }
                }
            }
        })
    }

    #[allow(dead_code)]
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}
