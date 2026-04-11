use crate::config::LauncherSettings;
use crate::events::InstallJobId;
use crate::proxy::HttpClientFactory;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub struct LauncherState {
    pub settings: Arc<RwLock<LauncherSettings>>,
    pub http: Arc<tokio::sync::RwLock<HttpClientFactory>>,
    pub jobs: Arc<JobRegistry>,
    /// Where settings.json lives. Set during `setup()` from
    /// `app.path().app_config_dir()`.
    pub config_dir: Arc<RwLock<PathBuf>>,
}

impl LauncherState {
    pub fn new(settings: LauncherSettings, http: HttpClientFactory) -> Self {
        Self {
            settings: Arc::new(RwLock::new(settings)),
            http: Arc::new(tokio::sync::RwLock::new(http)),
            jobs: Arc::new(JobRegistry::default()),
            config_dir: Arc::new(RwLock::new(PathBuf::new())),
        }
    }

    pub fn save_settings(&self) -> crate::error::AppResult<()> {
        let dir = self.config_dir.read().clone();
        let s = self.settings.read().clone();
        s.save(&dir)
    }
}

#[derive(Default)]
pub struct JobRegistry {
    inner: Mutex<HashMap<InstallJobId, JobHandle>>,
}

impl JobRegistry {
    pub fn insert(&self, id: InstallJobId, handle: JobHandle) {
        self.inner.lock().insert(id, handle);
    }
    pub fn cancel(&self, id: &str) -> bool {
        if let Some(h) = self.inner.lock().get(id) {
            h.cancel.cancel();
            // Unblock anyone waiting on the pause gate so a cancel-while-paused
            // wakes them up immediately — they'll then see the cancel token.
            h.pause.set_paused(false);
            return true;
        }
        false
    }
    /// Toggle the pause flag on an existing job. Returns false if the job is
    /// unknown (e.g. already finished).
    pub fn set_paused(&self, id: &str, paused: bool) -> bool {
        if let Some(h) = self.inner.lock().get(id) {
            h.pause.set_paused(paused);
            return true;
        }
        false
    }
    pub fn remove(&self, id: &str) {
        self.inner.lock().remove(id);
    }
}

#[derive(Clone)]
pub struct JobHandle {
    pub cancel: CancellationToken,
    pub pause: Arc<PauseState>,
}

/// Cooperative pause gate used by long-running install/update/repair jobs.
///
/// Workers call [`PauseState::wait`] between units of work (a file, a body
/// chunk, a scan entry). While `paused` is true, `wait` suspends them; flipping
/// it back to false wakes every waiter via a `Notify` broadcast. Cancelling a
/// job un-pauses it first (see [`JobRegistry::cancel`]) so a cancel always
/// wins over a pause.
pub struct PauseState {
    paused: AtomicBool,
    notify: Notify,
}

impl PauseState {
    pub fn new() -> Self {
        Self {
            paused: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }

    pub fn set_paused(&self, paused: bool) {
        let was = self.paused.swap(paused, Ordering::SeqCst);
        if was && !paused {
            // Wake every task currently awaiting `notified()`.
            self.notify.notify_waiters();
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Suspend the current task until the gate is open. Returns immediately
    /// if not currently paused.
    ///
    /// The double-check-around-`notified()` dance is required: we must register
    /// interest on the `Notify` BEFORE re-reading the atomic to avoid a missed
    /// wakeup between the load and the notified().await.
    pub async fn wait(&self) {
        loop {
            if !self.paused.load(Ordering::Relaxed) {
                return;
            }
            let notified = self.notify.notified();
            if !self.paused.load(Ordering::Relaxed) {
                return;
            }
            notified.await;
        }
    }
}

impl Default for PauseState {
    fn default() -> Self {
        Self::new()
    }
}
