use crate::error::{AppError, AppResult};
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{ProcessesToUpdate, System};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const LOG_NAMES: &[&str] = &[
    "message.log",
    "warning.log",
    "error.log",
    "script.log",
    "script_warning.log",
    "filesystem.log",
    "net_trace.log",
    "netconsole.log",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RiskyApplication {
    pub name: String,
    pub category: String,
    pub reason: String,
    pub process_name: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize)]
struct ProcessSnapshot {
    pid: u32,
    name: String,
    risk: Option<RiskSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct RiskSummary {
    name: String,
    category: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct SystemSnapshot {
    collected_at: String,
    username: Option<String>,
    os: String,
    kernel_version: Option<String>,
    boot_time: String,
    uptime_seconds: u64,
    cpu: String,
    logical_cpu_count: usize,
    total_memory_bytes: u64,
    available_memory_bytes: u64,
    gpus: Vec<String>,
    processes: Vec<ProcessSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReportResult {
    pub archive_path: String,
    pub game_log_directory: Option<String>,
    pub included_files: Vec<String>,
    pub missing_crash_files: Vec<String>,
    pub risky_applications: Vec<RiskyApplication>,
}

struct RiskRule {
    name: &'static str,
    category: &'static str,
    reason: &'static str,
    patterns: &'static [&'static str],
}

const RISK_RULES: &[RiskRule] = &[
    RiskRule {
        name: "Steam 游戏内覆盖",
        category: "悬浮层/性能监控",
        reason: "游戏内覆盖和性能图层会向渲染进程注入组件",
        patterns: &["gameoverlayui", "steamoverlay", "steam.exe"],
    },
    RiskRule {
        name: "EA App / Origin 游戏内覆盖",
        category: "悬浮层/性能监控",
        reason: "EA/Origin 游戏内覆盖组件可能与其他注入层冲突",
        patterns: &[
            "igo64",
            "igo32",
            "origin.exe",
            "originwebhelperservice",
            "eadesktop.exe",
        ],
    },
    RiskRule {
        name: "RivaTuner Statistics Server",
        category: "超频/性能监控",
        reason: "帧率监控会注入游戏渲染进程",
        patterns: &["rtss", "rivatuner"],
    },
    RiskRule {
        name: "MSI Afterburner",
        category: "超频/性能监控",
        reason: "硬件监控和悬浮显示可能影响游戏稳定性",
        patterns: &["msiafterburner", "afterburner"],
    },
    RiskRule {
        name: "HWiNFO",
        category: "超频/性能监控",
        reason: "传感器轮询和共享监控可能与悬浮层联动",
        patterns: &["hwinfo32", "hwinfo64", "hwinfo"],
    },
    RiskRule {
        name: "AIDA64",
        category: "超频/性能监控",
        reason: "传感器监控或屏显功能可能影响稳定性",
        patterns: &["aida64"],
    },
    RiskRule {
        name: "GPU Tweak",
        category: "超频/性能监控",
        reason: "显卡调校与屏显功能可能影响游戏稳定性",
        patterns: &["gputweak", "asusgputweak"],
    },
    RiskRule {
        name: "EVGA Precision",
        category: "超频/性能监控",
        reason: "显卡调校与屏显功能可能影响游戏稳定性",
        patterns: &["precisionx", "precision_x"],
    },
    RiskRule {
        name: "AMD Ryzen Master",
        category: "超频/性能监控",
        reason: "处理器调校或监控可能影响系统稳定性",
        patterns: &["ryzenmaster"],
    },
    RiskRule {
        name: "Intel XTU",
        category: "超频/性能监控",
        reason: "处理器调校或监控可能影响系统稳定性",
        patterns: &["xtuui", "xtuservice"],
    },
    RiskRule {
        name: "OBS Studio",
        category: "直播/录屏",
        reason: "游戏捕获会挂接图形接口",
        patterns: &["obs32", "obs64", "obs-studio"],
    },
    RiskRule {
        name: "Streamlabs",
        category: "直播/录屏",
        reason: "游戏捕获和悬浮组件会挂接图形接口",
        patterns: &["streamlabs"],
    },
    RiskRule {
        name: "XSplit",
        category: "直播/录屏",
        reason: "游戏捕获会挂接图形接口",
        patterns: &["xsplit"],
    },
    RiskRule {
        name: "哔哩哔哩直播姬",
        category: "直播/录屏",
        reason: "游戏捕获和弹幕悬浮层可能影响稳定性",
        patterns: &["livehime", "bilibili live"],
    },
    RiskRule {
        name: "斗鱼直播伴侣",
        category: "直播/录屏",
        reason: "游戏捕获和弹幕悬浮层可能影响稳定性",
        patterns: &["douyutool", "douyu"],
    },
    RiskRule {
        name: "虎牙直播",
        category: "直播/录屏",
        reason: "游戏捕获和弹幕悬浮层可能影响稳定性",
        patterns: &["huyaclient", "huya"],
    },
    RiskRule {
        name: "Wallpaper Engine",
        category: "动态壁纸",
        reason: "动态壁纸持续使用图形资源，可能与独占全屏或驱动冲突",
        patterns: &["wallpaper32", "wallpaper64", "wallpaper_engine"],
    },
    RiskRule {
        name: "Lively Wallpaper",
        category: "动态壁纸",
        reason: "动态壁纸持续使用图形资源，可能与独占全屏或驱动冲突",
        patterns: &["livelywpf", "lively.wallpaper", "lively"],
    },
    RiskRule {
        name: "Discord 游戏内覆盖",
        category: "悬浮层/性能监控",
        reason: "游戏内覆盖会向渲染进程注入组件",
        patterns: &["discord"],
    },
    RiskRule {
        name: "Overwolf",
        category: "悬浮层/性能监控",
        reason: "游戏内应用和悬浮层会向渲染进程注入组件",
        patterns: &["overwolf"],
    },
    RiskRule {
        name: "NVIDIA 游戏内覆盖",
        category: "悬浮层/性能监控",
        reason: "性能图层、滤镜或录制功能可能与其他注入层冲突",
        patterns: &["nvidia share", "nvcontainer", "nvidia app"],
    },
    RiskRule {
        name: "AMD Software 游戏内覆盖",
        category: "悬浮层/性能监控",
        reason: "性能图层或录制功能可能与其他注入层冲突",
        patterns: &["radeonsoftware", "amdow"],
    },
    RiskRule {
        name: "Xbox Game Bar",
        category: "悬浮层/录屏",
        reason: "游戏栏和后台录制会挂接游戏窗口",
        patterns: &["gamebar", "gamebarftserver"],
    },
    RiskRule {
        name: "Medal",
        category: "直播/录屏",
        reason: "自动剪辑和游戏捕获会挂接图形接口",
        patterns: &["medal"],
    },
    RiskRule {
        name: "Razer Cortex",
        category: "悬浮层/性能监控",
        reason: "游戏内图层和优化功能可能改变游戏运行环境",
        patterns: &["razercortex", "cortexlauncher"],
    },
    RiskRule {
        name: "NZXT CAM",
        category: "悬浮层/性能监控",
        reason: "硬件监控和游戏内图层可能影响稳定性",
        patterns: &["nzxt cam", "nzxtcam"],
    },
];

pub fn collect(install_dir: &Path, destination: &Path) -> AppResult<DiagnosticReportResult> {
    if destination
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        != Some(true)
    {
        return Err(AppError::InvalidPath(
            "诊断包保存路径必须以 .zip 结尾".to_string(),
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let (snapshot, risky_applications) = capture_system();
    let latest_log_dir = latest_log_directory(&install_dir.join("platform").join("logs"));
    let mut included_files = Vec::new();
    let mut found_crash_text = false;
    let mut found_minidump = false;

    let output = File::create(destination)?;
    let mut zip = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let json = serde_json::to_vec_pretty(&snapshot)?;
    write_bytes(&mut zip, "diagnostics/system-info.json", &json, options)?;
    let text = format_system_report(&snapshot, &risky_applications);
    write_bytes(
        &mut zip,
        "diagnostics/system-info.txt",
        text.as_bytes(),
        options,
    )?;

    if let Some(log_dir) = &latest_log_dir {
        let mut entries: Vec<_> = fs::read_dir(log_dir)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !is_relevant_game_log(&file_name) {
                continue;
            }
            let lower = file_name.to_ascii_lowercase();
            found_crash_text |= lower.starts_with("apex_crash") && lower.ends_with(".txt");
            found_minidump |= lower.starts_with("minidump") && lower.ends_with(".dmp");
            let archive_name = format!("game-logs/{file_name}");
            add_file(&mut zip, &path, &archive_name, options)?;
            included_files.push(file_name);
        }
    }

    zip.finish()
        .map_err(|e| AppError::other(format!("完成诊断包失败: {e}")))?;

    let mut missing_crash_files = Vec::new();
    if !found_crash_text {
        missing_crash_files.push("apex_crash.txt".to_string());
    }
    if !found_minidump {
        missing_crash_files.push("minidump.dmp".to_string());
    }

    Ok(DiagnosticReportResult {
        archive_path: destination.display().to_string(),
        game_log_directory: latest_log_dir.map(|path| path.display().to_string()),
        included_files,
        missing_crash_files,
        risky_applications,
    })
}

fn capture_system() -> (SystemSnapshot, Vec<RiskyApplication>) {
    let mut system = System::new_all();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system.refresh_cpu_all();
    system.refresh_memory();

    let mut risky_applications = Vec::new();
    let mut seen_risky_applications = HashSet::new();
    let mut processes: Vec<ProcessSnapshot> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy().trim().to_string();
            if name.is_empty() {
                return None;
            }
            let risk = classify_process(&name).map(|rule| {
                let summary = RiskSummary {
                    name: rule.name.to_string(),
                    category: rule.category.to_string(),
                    reason: rule.reason.to_string(),
                };
                if seen_risky_applications.insert(rule.name) {
                    risky_applications.push(RiskyApplication {
                        name: rule.name.to_string(),
                        category: rule.category.to_string(),
                        reason: rule.reason.to_string(),
                        process_name: name.clone(),
                        pid: pid.as_u32(),
                    });
                }
                summary
            });
            Some(ProcessSnapshot {
                pid: pid.as_u32(),
                name,
                risk,
            })
        })
        .collect();

    processes.sort_by(|a, b| {
        b.risk
            .is_some()
            .cmp(&a.risk.is_some())
            .then_with(|| {
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase())
            })
            .then_with(|| a.pid.cmp(&b.pid))
    });
    risky_applications.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.pid.cmp(&b.pid))
    });

    let boot_time = DateTime::<Utc>::from_timestamp(System::boot_time() as i64, 0)
        .map(|value| value.with_timezone(&Local).to_rfc3339())
        .unwrap_or_else(|| System::boot_time().to_string());
    let cpu = system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());
    let os = [System::name(), System::long_os_version()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");

    (
        SystemSnapshot {
            collected_at: Local::now().to_rfc3339(),
            username: std::env::var("USERNAME")
                .ok()
                .or_else(|| std::env::var("USER").ok())
                .filter(|value| !value.trim().is_empty()),
            os: if os.is_empty() {
                "未知".to_string()
            } else {
                os
            },
            kernel_version: System::kernel_version(),
            boot_time,
            uptime_seconds: System::uptime(),
            cpu,
            logical_cpu_count: system.cpus().len(),
            total_memory_bytes: system.total_memory(),
            available_memory_bytes: system.available_memory(),
            gpus: detect_gpus(),
            processes,
        },
        risky_applications,
    )
}

fn classify_process(process_name: &str) -> Option<&'static RiskRule> {
    let lower = process_name.to_ascii_lowercase();
    RISK_RULES
        .iter()
        .find(|rule| rule.patterns.iter().any(|pattern| lower.contains(pattern)))
}

fn latest_log_directory(log_root: &Path) -> Option<PathBuf> {
    if !log_root.is_dir() {
        return None;
    }
    let latest_child = fs::read_dir(log_root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .max_by_key(|entry| modified_key(&entry.path()))
        .map(|entry| entry.path());
    latest_child.or_else(|| {
        fs::read_dir(log_root)
            .ok()?
            .filter_map(Result::ok)
            .any(|entry| entry.path().is_file())
            .then(|| log_root.to_path_buf())
    })
}

fn modified_key(path: &Path) -> u128 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn is_relevant_game_log(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    LOG_NAMES.contains(&lower.as_str())
        || (lower.starts_with("apex_crash") && lower.ends_with(".txt"))
        || (lower.starts_with("minidump") && lower.ends_with(".dmp"))
}

fn write_bytes(
    zip: &mut ZipWriter<File>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> AppResult<()> {
    zip.start_file(name, options)
        .map_err(|e| AppError::other(format!("写入诊断包失败: {e}")))?;
    zip.write_all(bytes)?;
    Ok(())
}

fn add_file(
    zip: &mut ZipWriter<File>,
    source: &Path,
    archive_name: &str,
    options: SimpleFileOptions,
) -> AppResult<()> {
    zip.start_file(archive_name, options)
        .map_err(|e| AppError::other(format!("写入诊断包失败: {e}")))?;
    let mut input = File::open(source)?;
    io::copy(&mut input, zip)?;
    Ok(())
}

fn format_system_report(
    snapshot: &SystemSnapshot,
    risky_applications: &[RiskyApplication],
) -> String {
    let mut output = String::new();
    output.push_str("R5R 崩溃诊断\n");
    output.push_str("================\n");
    output.push_str(&format!("收集时间: {}\n", snapshot.collected_at));
    output.push_str(&format!(
        "Windows 用户: {}\n",
        snapshot.username.as_deref().unwrap_or("未知")
    ));
    output.push_str(&format!("操作系统: {}\n", snapshot.os));
    output.push_str(&format!("开机时间: {}\n", snapshot.boot_time));
    output.push_str(&format!("运行时长: {} 秒\n", snapshot.uptime_seconds));
    output.push_str(&format!(
        "CPU: {} ({} 逻辑处理器)\n",
        snapshot.cpu, snapshot.logical_cpu_count
    ));
    output.push_str(&format!(
        "内存: 总计 {:.2} GiB / 可用 {:.2} GiB\n",
        snapshot.total_memory_bytes as f64 / 1_073_741_824.0,
        snapshot.available_memory_bytes as f64 / 1_073_741_824.0
    ));
    output.push_str(&format!(
        "显卡: {}\n",
        if snapshot.gpus.is_empty() {
            "未知".to_string()
        } else {
            snapshot.gpus.join("；")
        }
    ));

    output.push_str("\n可能影响游戏稳定性的应用（优先排查）\n");
    output.push_str("------------------------------------\n");
    if risky_applications.is_empty() {
        output.push_str("未检测到已知的悬浮、监控、超频、直播或动态壁纸应用。\n");
    } else {
        for app in risky_applications {
            output.push_str(&format!(
                "[{}] {} - {} ({}，PID {})\n",
                app.category, app.name, app.reason, app.process_name, app.pid
            ));
        }
    }

    output.push_str("\n全部进程（风险项已排在前面）\n");
    output.push_str("----------------------------\n");
    for process in &snapshot.processes {
        let marker = if process.risk.is_some() {
            "[需排查] "
        } else {
            ""
        };
        output.push_str(&format!("{marker}{} (PID {})\n", process.name, process.pid));
    }
    output
}

#[cfg(windows)]
fn detect_gpus() -> Vec<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(video) = hklm.open_subkey(r"SYSTEM\CurrentControlSet\Control\Video") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for guid in video.enum_keys().flatten() {
        let Ok(guid_key) = video.open_subkey(guid) else {
            continue;
        };
        for adapter in guid_key.enum_keys().flatten() {
            let Ok(adapter_key) = guid_key.open_subkey(adapter) else {
                continue;
            };
            let name: Option<String> = adapter_key.get_value("DriverDesc").ok().or_else(|| {
                adapter_key
                    .get_value("HardwareInformation.AdapterString")
                    .ok()
            });
            let Some(name) = name.map(|value| value.trim().to_string()) else {
                continue;
            };
            if name.is_empty() || !seen.insert(name.to_ascii_lowercase()) {
                continue;
            }
            found.push(name);
        }
    }
    found.sort();
    found
}

#[cfg(not(windows))]
fn detect_gpus() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn classifies_common_injected_applications() {
        assert_eq!(
            classify_process("MSIAfterburner.exe").map(|rule| rule.name),
            Some("MSI Afterburner")
        );
        assert_eq!(
            classify_process("obs64.exe").map(|rule| rule.category),
            Some("直播/录屏")
        );
        assert_eq!(classify_process("explorer.exe").map(|rule| rule.name), None);
    }

    #[test]
    fn selects_newest_log_session() {
        let temp = tempdir().unwrap();
        let older = temp.path().join("older");
        let newer = temp.path().join("newer");
        fs::create_dir(&older).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::create_dir(&newer).unwrap();
        assert_eq!(latest_log_directory(temp.path()), Some(newer));
    }

    #[test]
    fn recognizes_sdk_crash_and_session_logs() {
        assert!(is_relevant_game_log("apex_crash.txt"));
        assert!(is_relevant_game_log("apex_crash_20260728_120000.txt"));
        assert!(is_relevant_game_log("minidump.dmp"));
        assert!(is_relevant_game_log("warning.log"));
        assert!(!is_relevant_game_log("unrelated.bin"));
    }
}
