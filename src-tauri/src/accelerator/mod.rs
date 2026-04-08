//! Game-accelerator (VPN) detection.
//!
//! Chinese gamers commonly run an "加速器" (UU / 奇游 / 迅游 / 雷神 etc.) to
//! tunnel game traffic through the publisher's edge nodes. For *official*
//! Apex this is helpful, but for the **community-server** mirror it routes
//! traffic through an unrelated tunnel that has no idea about R5R servers
//! — the result is packet loss, jitter, and disconnects. We scan the
//! running process list at startup (and periodically thereafter) and warn
//! the user before they hit "启动游戏".
//!
//! Detection is best-effort: we match by process name substring, so a
//! renamed binary will slip through. The catalog below is intentionally
//! broad — if there's a false positive the user just clicks "继续启动".

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetectedAccelerator {
    /// Friendly name shown in the UI (e.g. `"UU加速器"`).
    pub name: String,
    /// The actual process name we matched on (lowercase).
    pub process_name: String,
    pub pid: u32,
}

/// (friendly_name, &[lowercase substring patterns]).
///
/// Patterns are matched case-insensitively against the process basename.
/// Order matters only for first-match wins per (friendly_name) bucket —
/// the same friendly name is reported once even if multiple helper
/// processes are running.
const KNOWN: &[(&str, &[&str])] = &[
    // 网易 UU 加速器 — main exe is `uu.exe`, helpers `netease_uu_helper.exe`,
    // `uuwebview.exe`. The `uu.exe` substring is risky (collides with
    // anything containing "uu"), so we keep it tight to executables.
    ("UU加速器", &["netease_uu", "uuwebview", "uubooster"]),
    ("奇游加速器", &["qiyou", "qygame", "qyjsq"]),
    ("迅游加速器", &["xunyou", "xyjsq"]),
    ("雷神加速器", &["leitingjsq", "leishen", "thunderacc"]),
    ("海豚加速器", &["dolphinacc", "haitunjsq"]),
    ("腾讯网游加速器", &["qqgameacc", "tencentacc", "wegameacc"]),
    ("AK加速器", &["akjsq", "akacc", "akjiasu"]),
    ("VK加速器", &["vkjsq", "vkacc", "vkjiasu"]),
    ("熊猫加速器", &["pandagame", "xiongmao_jsq"]),
    ("古怪加速器", &["guguai", "ggjsq"]),
    ("加速精灵", &["jiasujingling"]),
    ("NN加速器", &["nnjsq", "nnacc"]),
    // Generic: catch-all for the substring "加速器" itself, useful when
    // a Chinese vendor ships a binary with the literal Chinese name.
    ("未知加速器", &["jiasuqi"]),
];

/// Enumerate running processes and return any matched accelerators.
/// Cheap enough to call from an IPC command synchronously (~5–20ms on a
/// typical Windows box). Returns an empty vec on failure.
pub fn detect() -> Vec<DetectedAccelerator> {
    use sysinfo::{ProcessesToUpdate, System};

    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let mut found: Vec<DetectedAccelerator> = Vec::new();
    let mut seen_friendly: HashSet<&'static str> = HashSet::new();

    for (pid, process) in sys.processes() {
        let name_os = process.name();
        let pname = name_os.to_string_lossy().to_ascii_lowercase();
        if pname.is_empty() {
            continue;
        }
        for (friendly, patterns) in KNOWN {
            if seen_friendly.contains(friendly) {
                continue;
            }
            if patterns.iter().any(|p| pname.contains(*p)) {
                seen_friendly.insert(*friendly);
                found.push(DetectedAccelerator {
                    name: (*friendly).to_string(),
                    process_name: pname.clone(),
                    pid: pid.as_u32(),
                });
                break;
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_runs_without_panicking() {
        // Smoke test: scanning the dev machine should at minimum not crash.
        // We don't assert on results because the dev box may or may not be
        // running an accelerator.
        let _ = detect();
    }
}
