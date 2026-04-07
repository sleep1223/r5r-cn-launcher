use crate::config::LauncherSettings;
use crate::events::InstallJobId;
use crate::proxy::HttpClientFactory;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
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
            return true;
        }
        false
    }
    pub fn remove(&self, id: &str) {
        self.inner.lock().remove(id);
    }
}

pub struct JobHandle {
    pub cancel: CancellationToken,
}
