use crate::launch_options::catalog::catalog;
use crate::launch_options::model::*;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningSeverity {
    Info,
    Caution,
    Danger,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchWarning {
    pub severity: WarningSeverity,
    pub message_zh: String,
    pub related_option_ids: Vec<String>,
}

const SAFE_RES: &[(i32, i32)] = &[
    (1280, 720),
    (1366, 768),
    (1600, 900),
    (1920, 1080),
    (2560, 1440),
    (3840, 2160),
];

pub fn validate_launch_args(selection: &LaunchOptionSelection) -> Vec<LaunchWarning> {
    let cat = catalog();
    let mut warns = Vec::new();

    let is_enabled = |id: &str| -> bool {
        if let Some(item) = selection.items.get(id) {
            return item.enabled;
        }
        cat.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.default_enabled)
            .unwrap_or(false)
    };

    // 1. Non-native resolution warning.
    if is_enabled("resolution") {
        let res = selection
            .items
            .get("resolution")
            .and_then(|i| i.value.clone())
            .or_else(|| {
                cat.entries
                    .iter()
                    .find(|e| e.id == "resolution")
                    .and_then(|e| e.default_value.clone())
            });
        if let Some(OptionValue::IntPair(w, h)) = res {
            if !SAFE_RES.contains(&(w, h)) {
                warns.push(LaunchWarning {
                    severity: WarningSeverity::Caution,
                    message_zh: format!(
                        "{}x{} 不是常见原生分辨率，可能导致游戏无法启动；如需使用，请先在显卡驱动控制面板中添加自定义分辨率。",
                        w, h
                    ),
                    related_option_ids: vec!["resolution".into()],
                });
            }
        }
    }

    // 2. Generic conflicts_with walk.
    use std::collections::HashSet;
    let mut reported: HashSet<(String, String)> = HashSet::new();
    for entry in &cat.entries {
        if !is_enabled(entry.id) {
            continue;
        }
        for c in entry.conflicts_with {
            if !is_enabled(c) {
                continue;
            }
            let mut pair = [entry.id.to_string(), (*c).to_string()];
            pair.sort();
            let key = (pair[0].clone(), pair[1].clone());
            if reported.contains(&key) {
                continue;
            }
            reported.insert(key.clone());
            warns.push(LaunchWarning {
                severity: WarningSeverity::Caution,
                message_zh: format!(
                    "「{}」与「{}」可能冲突，建议只启用其中一个。",
                    label_for(entry.id),
                    label_for(c)
                ),
                related_option_ids: vec![pair[0].clone(), pair[1].clone()],
            });
        }
    }

    warns
}

fn label_for(id: &str) -> &'static str {
    catalog()
        .entries
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.label_zh)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn defaults_have_no_warnings() {
        let sel = LaunchOptionSelection { items: HashMap::new() };
        let w = validate_launch_args(&sel);
        assert_eq!(w.len(), 0);
    }

    #[test]
    fn non_native_resolution_warns() {
        let mut items = HashMap::new();
        items.insert(
            "resolution".to_string(),
            SelectionEntry {
                enabled: true,
                value: Some(OptionValue::IntPair(1234, 567)),
            },
        );
        let sel = LaunchOptionSelection { items };
        let w = validate_launch_args(&sel);
        assert_eq!(w.len(), 1);
        assert!(w[0].message_zh.contains("1234x567"));
    }

    #[test]
    fn fps_max_low_input_latency_conflict_detected() {
        // Window mode is now a single EnumArgs entry (no internal conflict),
        // so the remaining conflict pair the catalog declares is fps_max +
        // no_render_on_input_thread. Use that as the regression target.
        let mut items = HashMap::new();
        items.insert(
            "fps_max".to_string(),
            SelectionEntry {
                enabled: true,
                value: Some(OptionValue::Int(144)),
            },
        );
        items.insert(
            "no_render_on_input_thread".to_string(),
            SelectionEntry { enabled: true, value: None },
        );
        let sel = LaunchOptionSelection { items };
        let w = validate_launch_args(&sel);
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn native_resolution_no_warning() {
        let mut items = HashMap::new();
        items.insert(
            "resolution".to_string(),
            SelectionEntry {
                enabled: true,
                value: Some(OptionValue::IntPair(1920, 1080)),
            },
        );
        let sel = LaunchOptionSelection { items };
        let w = validate_launch_args(&sel);
        assert_eq!(w.len(), 0);
    }
}
