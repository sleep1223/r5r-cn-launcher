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
    /// `["+cl_fovScale", "1.7"]` or `["-fullscreen"]`.
    Toggle { args: &'static [&'static str] },
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
    /// Free-text string, emits `[flag, value]`.
    String {
        flag: &'static str,
        placeholder: &'static str,
    },
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
