use serde::{Deserialize, Serialize};

#[cfg(windows)]
use crate::config::paths::LIBRARY_DIR_NAME;

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
    /// True when `<path>/r5apex.exe` exists — i.e. launching directly from
    /// `path` would work. Library-scan hits are already filtered on this, but
    /// registry/shortcut hits can point at a library root or a parent dir
    /// that isn't itself runnable.
    #[serde(default)]
    pub has_game: bool,
}

#[cfg(windows)]
pub async fn detect_existing(extra_roots: &[String]) -> Vec<DetectedInstall> {
    let (registry_hits, shortcut_hits, scan_hits) = tokio::join!(
        async { registry::detect().unwrap_or_default() },
        async { shortcut::detect().unwrap_or_default() },
        async { library_scan::detect(extra_roots) },
    );
    let mut all = Vec::new();
    // Prefer the uninstall registry as the canonical source. Only fall back
    // to the Start Menu shortcut if the registry has nothing — we don't want
    // two near-duplicate rows showing the same install.
    if !registry_hits.is_empty() {
        all.extend(registry_hits);
    } else {
        all.extend(shortcut_hits);
    }
    all.extend(scan_hits);

    // Registry/shortcut hits point at the *install base* (e.g.
    // `D:\Program Files\R5Reloaded`), not at the runnable channel dir. The
    // official R5Reloaded layout is always `<base>\R5R Library\<CHANNEL>\r5apex.exe`,
    // so probe that sub-structure for each hit and replace a base-path hit with
    // one entry per channel that actually contains r5apex.exe.
    let expanded = expand_into_channels(all);
    dedupe(expanded)
}

#[cfg(windows)]
fn expand_into_channels(hits: Vec<DetectedInstall>) -> Vec<DetectedInstall> {
    let mut out = Vec::new();
    for hit in hits {
        let base = std::path::Path::new(&hit.path);
        // Case 1: hit already points at a channel dir (library_scan does this).
        if base.join("r5apex.exe").is_file() {
            let mut h = hit;
            h.has_game = true;
            out.push(h);
            continue;
        }
        // Case 2: hit points at the install base. Probe `<base>\R5R Library\<CHANNEL>\`.
        let lib = base.join(LIBRARY_DIR_NAME);
        let mut expanded_any = false;
        if let Ok(rd) = std::fs::read_dir(&lib) {
            for entry in rd.flatten() {
                let channel_dir = entry.path();
                if !channel_dir.is_dir() {
                    continue;
                }
                if !channel_dir.join("r5apex.exe").is_file() {
                    continue;
                }
                expanded_any = true;
                let channel = channel_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned());
                out.push(DetectedInstall {
                    source: hit.source.clone(),
                    path: channel_dir.display().to_string(),
                    channel,
                    version: hit.version.clone(),
                    has_game: true,
                });
            }
        }
        if !expanded_any {
            // Keep the original hit so it still shows in the "detected" list,
            // but with has_game=false so the UI won't try to launch from here.
            let mut h = hit;
            h.has_game = false;
            out.push(h);
        }
    }
    out
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
        // Normalize separators so mixed forward/backslash paths dedupe.
        let key = d.path.to_ascii_lowercase().replace('/', "\\");
        seen.insert(key)
    });
    v
}
