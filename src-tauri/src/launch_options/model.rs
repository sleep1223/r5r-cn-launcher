use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct LaunchOptionCatalog {
    pub categories: Vec<Category>,
    pub entries: Vec<OptionEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Category {
    pub id: &'static str,
    pub label_zh: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptionEntry {
    pub id: &'static str,
    pub category: &'static str,
    pub kind: OptionKind,
    pub default_enabled: bool,
    pub default_value: Option<OptionValue>,
    pub label_zh: &'static str,
    pub description_zh: &'static str,
    pub risk: RiskLevel,
    pub conflicts_with: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OptionKind {
    /// Toggle that emits a fixed sequence of args when enabled, e.g.
    /// `["+cl_fovScale", "1.7"]`, `["-fullscreen"]`, or the mouse-optimize
    /// combo `["+m_rawinput", "1", "-noforcemaccel", "-noforcemspd",
    /// "-noforcemparms"]`. Set `is_combo` so the UI can render a "rich"
    /// expandable description block instead of a plain checkbox.
    Toggle {
        args: &'static [&'static str],
        #[serde(default)]
        is_combo: bool,
    },
    /// Single int value, emits `[flag, value]`.
    Int {
        flag: &'static str,
        min: i32,
        max: i32,
    },
    /// Single decimal value (FOV scale, letterbox aspect ratio, etc.).
    /// Emits `[flag, value]` with the value formatted via `format!("{}")`,
    /// which strips trailing zeros (`1.70 → "1.7"`).
    Float {
        flag: &'static str,
        min: f64,
        max: f64,
        step: f64,
    },
    /// Resolution-style: emits `[x_flag, width, y_flag, height]`.
    IntPair {
        x_flag: &'static str,
        y_flag: &'static str,
    },
    /// Enum dropdown, emits `[flag, value]`.
    Enum {
        flag: &'static str,
        choices: &'static [(&'static str, &'static str)], // (value, label_zh)
    },
    /// Mutually exclusive choice that emits a *full arg sequence* per pick.
    /// Used for the combined "窗口模式" entry where each option emits a
    /// different flag (e.g. `-fullscreen` vs `-noborder -window`). The
    /// selected value is `OptionValue::Enum(value_id)` and compose pushes
    /// the matching `args` slice verbatim.
    EnumArgs {
        choices: &'static [EnumArgChoice],
    },
    /// FOV-style: stored as **integer degrees** (e.g. 70..=120), emitted as
    /// the float scale `degrees / base` (Apex's `+cl_fovScale 1.0` = 70°,
    /// 1.7 ≈ 120°). The user thinks in degrees; the wire format is the
    /// scale ratio, and we convert at compose time.
    FovDegrees {
        flag: &'static str,
        min: i32,
        max: i32,
        base: i32,
    },
    /// Free-text string, emits `[flag, value]`.
    String {
        flag: &'static str,
        placeholder: &'static str,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct EnumArgChoice {
    pub value: &'static str,
    pub label_zh: &'static str,
    pub args: &'static [&'static str],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum OptionValue {
    Bool(bool),
    Int(i32),
    Float(f64),
    IntPair(i32, i32),
    Enum(String),
    String(String),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    None,
    Caution,
    Danger,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LaunchOptionSelection {
    pub items: HashMap<String, SelectionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionEntry {
    pub enabled: bool,
    #[serde(default)]
    pub value: Option<OptionValue>,
}
