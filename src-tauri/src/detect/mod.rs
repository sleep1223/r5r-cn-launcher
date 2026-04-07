use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod library_scan;
#[cfg(windows)]
mod registry;
#[cfg(windows)]
mod shortcut;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DetectSource {
    Shortcut,
    Registry,
    LibraryScan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedInstall {
    pub source: DetectSource,
    pub path: String,
    pub channel: Option<String>,
    pub version: Option<String>,
}

#[cfg(windows)]
pub async fn detect_existing(extra_roots: &[String]) -> Vec<DetectedInstall> {
    let (a, b, c) = tokio::join!(
        async { shortcut::detect().unwrap_or_default() },
        async { registry::detect().unwrap_or_default() },
        async { library_scan::detect(extra_roots) },
    );
    let mut all = Vec::new();
    all.extend(a);
    all.extend(b);
    all.extend(c);
    dedupe(all)
}

#[cfg(not(windows))]
pub async fn detect_existing(_extra_roots: &[String]) -> Vec<DetectedInstall> {
    // Detection is Windows-only — R5Reloaded doesn't run on macOS/Linux.
    Vec::new()
}

#[cfg(windows)]
fn dedupe(mut v: Vec<DetectedInstall>) -> Vec<DetectedInstall> {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = HashSet::new();
    v.retain(|d| {
        let key = d.path.to_ascii_lowercase();
        seen.insert(key)
    });
    v
}
