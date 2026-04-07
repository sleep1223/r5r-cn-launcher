use crate::detect::{DetectSource, DetectedInstall};
use std::path::PathBuf;

const DEFAULT_ROOTS: &[&str] = &[
    r"C:\Program Files\R5R Library",
    r"C:\R5R Library",
    r"D:\R5R Library",
    r"E:\R5R Library",
    r"F:\R5R Library",
];

/// Scan the default library locations (and any extras the caller passes in)
/// looking for `<root>/<channel>/r5apex.exe`.
pub fn detect(extra_roots: &[String]) -> Vec<DetectedInstall> {
    let mut roots: Vec<PathBuf> = DEFAULT_ROOTS.iter().map(PathBuf::from).collect();
    for r in extra_roots {
        if r.is_empty() {
            continue;
        }
        // The library_root in settings is the *parent* of `R5R Library/`.
        let p = PathBuf::from(r).join("R5R Library");
        roots.push(p);
        roots.push(PathBuf::from(r));
    }

    let mut out = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let exe = path.join("r5apex.exe");
            if exe.exists() {
                let channel = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());
                out.push(DetectedInstall {
                    source: DetectSource::LibraryScan,
                    path: path.display().to_string(),
                    channel,
                    version: None,
                });
            }
        }
    }
    out
}
