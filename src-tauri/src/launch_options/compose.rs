use crate::launch_options::catalog::catalog;
use crate::launch_options::model::*;

/// Convert a `LaunchOptionSelection` into the final argv list for `r5apex.exe`.
///
/// Walks the catalog in order, pulls out enabled entries (using user overrides
/// when present, defaults otherwise), and appends each entry's args.
pub fn compose_launch_args(selection: &LaunchOptionSelection) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let cat = catalog();

    for entry in &cat.entries {
        let item = selection.items.get(entry.id);
        let enabled = item.map(|i| i.enabled).unwrap_or(entry.default_enabled);
        if !enabled {
            continue;
        }
        let value = item
            .and_then(|i| i.value.clone())
            .or_else(|| entry.default_value.clone());

        match &entry.kind {
            OptionKind::Toggle { args, .. } => {
                for a in *args {
                    out.push((*a).to_string());
                }
            }
            OptionKind::EnumArgs { choices } => {
                let v = match value {
                    Some(OptionValue::Enum(s)) => s,
                    _ => continue,
                };
                if let Some(c) = choices.iter().find(|c| c.value == v) {
                    for a in c.args {
                        out.push((*a).to_string());
                    }
                }
            }
            OptionKind::FovDegrees { flag, base, .. } => {
                let degrees = match value {
                    Some(OptionValue::Int(n)) => n,
                    _ => continue,
                };
                out.push((*flag).to_string());
                // Format with up to 4 decimals, then strip trailing zeros so
                // 70°→"1", 120°→"1.7143", 100°→"1.4286". Apex's parser
                // accepts plain decimals.
                let scale = degrees as f64 / *base as f64;
                let s = format!("{:.4}", scale);
                let trimmed = s.trim_end_matches('0').trim_end_matches('.').to_string();
                out.push(if trimmed.is_empty() { "0".to_string() } else { trimmed });
            }
            OptionKind::Int { flag, .. } => {
                let v = match value {
                    Some(OptionValue::Int(n)) => n,
                    _ => continue,
                };
                out.push((*flag).to_string());
                out.push(v.to_string());
            }
            OptionKind::Float { flag, .. } => {
                let v = match value {
                    Some(OptionValue::Float(n)) => n,
                    _ => continue,
                };
                out.push((*flag).to_string());
                // `format!("{}", 1.7_f64)` yields "1.7" — short, exact, no
                // scientific notation in the ranges we deal with (~0.5 .. 5.0).
                out.push(format!("{}", v));
            }
            OptionKind::IntPair { x_flag, y_flag } => {
                let (w, h) = match value {
                    Some(OptionValue::IntPair(w, h)) => (w, h),
                    _ => continue,
                };
                out.push((*x_flag).to_string());
                out.push(w.to_string());
                out.push((*y_flag).to_string());
                out.push(h.to_string());
            }
            OptionKind::Enum { flag, .. } => {
                let v = match value {
                    Some(OptionValue::Enum(s)) => s,
                    _ => continue,
                };
                out.push((*flag).to_string());
                out.push(v);
            }
            OptionKind::String { flag, .. } => {
                let v = match value {
                    Some(OptionValue::String(s)) if !s.is_empty() => s,
                    _ => continue,
                };
                out.push((*flag).to_string());
                out.push(v);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn defaults_compose_to_chinese_pylon_and_novid() {
        // Defaults must stay locked: language, pylon hostname, then -novid
        // (skip-intro is default-on as of v0.4 — see catalog.rs).
        let sel = LaunchOptionSelection { items: HashMap::new() };
        let args = compose_launch_args(&sel);
        assert_eq!(
            args,
            vec![
                "-language",
                "schinese",
                "+pylon_matchmaking_hostname",
                "r5r-org.sleep0.de",
                "-novid",
            ]
        );
    }

    #[test]
    fn disabling_default_omits_it() {
        let mut items = HashMap::new();
        items.insert(
            "language".to_string(),
            SelectionEntry { enabled: false, value: None },
        );
        let sel = LaunchOptionSelection { items };
        let args = compose_launch_args(&sel);
        assert!(!args.contains(&"-language".to_string()));
        // pylon default still present
        assert!(args.contains(&"+pylon_matchmaking_hostname".to_string()));
    }

    #[test]
    fn resolution_emits_w_and_h() {
        let mut items = HashMap::new();
        items.insert(
            "resolution".to_string(),
            SelectionEntry {
                enabled: true,
                value: Some(OptionValue::IntPair(1920, 1080)),
            },
        );
        let sel = LaunchOptionSelection { items };
        let args = compose_launch_args(&sel);
        assert!(args.windows(4).any(|w| w == ["-w", "1920", "-h", "1080"]));
    }

    #[test]
    fn window_mode_noborder_window_emits_both_flags() {
        let mut items = HashMap::new();
        items.insert(
            "window_mode".to_string(),
            SelectionEntry {
                enabled: true,
                value: Some(OptionValue::Enum("noborder_window".into())),
            },
        );
        let sel = LaunchOptionSelection { items };
        let args = compose_launch_args(&sel);
        assert!(args.iter().any(|a| a == "-noborder"));
        assert!(args.iter().any(|a| a == "-window"));
        assert!(!args.iter().any(|a| a == "-fullscreen"));
    }

    #[test]
    fn fov_degrees_emit_scale_value() {
        // 70° → 1.0 (default), 120° → 120/70 ≈ 1.7143, 100° → 100/70 ≈ 1.4286
        for (deg, expected) in &[(70, "1"), (90, "1.2857"), (100, "1.4286"), (120, "1.7143")] {
            let mut items = HashMap::new();
            items.insert(
                "fov_scale".to_string(),
                SelectionEntry {
                    enabled: true,
                    value: Some(OptionValue::Int(*deg)),
                },
            );
            let sel = LaunchOptionSelection { items };
            let args = compose_launch_args(&sel);
            let pos = args
                .iter()
                .position(|a| a == "+cl_fovScale")
                .expect("+cl_fovScale should be present");
            assert_eq!(args[pos + 1], *expected, "deg={}", deg);
        }
    }

    #[test]
    fn mouse_optimize_combo_emits_all_four_args() {
        let mut items = HashMap::new();
        items.insert(
            "mouse_optimize".to_string(),
            SelectionEntry { enabled: true, value: None },
        );
        let sel = LaunchOptionSelection { items };
        let args = compose_launch_args(&sel);
        // The combo expands to +m_rawinput 1 -noforcemaccel -noforcemspd -noforcemparms.
        assert!(args.windows(2).any(|w| w == ["+m_rawinput", "1"]));
        assert!(args.iter().any(|a| a == "-noforcemaccel"));
        assert!(args.iter().any(|a| a == "-noforcemspd"));
        assert!(args.iter().any(|a| a == "-noforcemparms"));
    }
}
