use crate::launch_options::model::*;
use once_cell::sync::Lazy;

/// The full launch-option catalog. Authored by hand from
/// `mxtools/src/data/apex_launch_options_config.ts` (the source of truth for
/// the entries) plus the CN-launcher defaults at the top.
///
/// **Ordering matters:** the unit test in `compose.rs` locks in
/// `["-language", "schinese", "+pylon_matchmaking_hostname", ...]` as the
/// default-compose output, so the language + pylon entries MUST stay first
/// (and stay enabled-by-default).
pub fn catalog() -> &'static LaunchOptionCatalog {
    &CATALOG
}

static CATALOG: Lazy<LaunchOptionCatalog> = Lazy::new(|| LaunchOptionCatalog {
    categories: vec![
        Category { id: "language",    label_zh: "语言与体验" },
        Category { id: "display",     label_zh: "画面与显示" },
        Category { id: "performance", label_zh: "性能与帧率" },
        Category { id: "input",       label_zh: "操作与输入" },
        Category { id: "voice",       label_zh: "配音语言" },
    ],
    entries: vec![
        // ===== 语言与体验 — defaults at the top so the compose order stays
        // `-language schinese +pylon_matchmaking_hostname r5r-org.sleep0.de`
        // (locked in by `compose::tests::defaults_compose_to_chinese_and_pylon`).
        OptionEntry {
            id: "language",
            category: "language",
            kind: OptionKind::Enum {
                flag: "-language",
                choices: &[
                    ("schinese", "简体中文"),
                    ("tchinese", "繁体中文"),
                    ("english",  "English"),
                ],
            },
            default_enabled: true,
            default_value: Some(OptionValue::Enum("schinese".into())),
            label_zh: "界面语言",
            description_zh: "社区服界面语言。默认简体中文。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "pylon_hostname",
            category: "language",
            kind: OptionKind::String {
                flag: "+pylon_matchmaking_hostname",
                placeholder: "r5r-org.sleep0.de",
            },
            default_enabled: true,
            default_value: Some(OptionValue::String("r5r-org.sleep0.de".into())),
            label_zh: "镜像服务器列表",
            description_zh: "Pylon 匹配服务器主机名。社区服默认使用 r5r-org.sleep0.de。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "skip_intro",
            category: "language",
            kind: OptionKind::Toggle { args: &["-novid"], is_combo: false },
            default_enabled: true,
            default_value: None,
            label_zh: "跳过开场动画",
            description_zh: "省去开场视频，约快 5 秒。默认开启。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "softened_locale",
            category: "language",
            kind: OptionKind::Toggle { args: &["+cl_is_softened_locale", "1"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "击杀血雾改红光",
            description_zh: "中文版默认效果：击倒敌人时闪一下红光。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },

        // ===== 画面与显示 =====
        OptionEntry {
            id: "window_mode",
            category: "display",
            kind: OptionKind::EnumArgs {
                choices: &[
                    EnumArgChoice { value: "fullscreen",       label_zh: "全屏",       args: &["-fullscreen"] },
                    EnumArgChoice { value: "window",           label_zh: "窗口",       args: &["-window"] },
                    EnumArgChoice { value: "noborder",         label_zh: "无边框",     args: &["-noborder"] },
                    EnumArgChoice { value: "noborder_window", label_zh: "无边框窗口", args: &["-noborder", "-window"] },
                ],
            },
            default_enabled: false,
            default_value: Some(OptionValue::Enum("fullscreen".into())),
            label_zh: "窗口模式",
            description_zh: "三种启动方式互斥：全屏 / 窗口 / 无边框。无边框窗口 = 同时使用 -noborder 与 -window。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "resolution",
            category: "display",
            kind: OptionKind::IntPair { x_flag: "-w", y_flag: "-h" },
            default_enabled: false,
            default_value: Some(OptionValue::IntPair(1920, 1080)),
            label_zh: "强制分辨率",
            description_zh:
                "以启动项设置游戏内分辨率。可在右侧选择常见预设（1280×720、1920×1080、2560×1440 等），也可手动输入。非原生分辨率可能导致游戏无法启动，必要时请先在显卡驱动中添加自定义分辨率。",
            risk: RiskLevel::Caution,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "aspect_min",
            category: "display",
            // Aspect is intentionally a fixed enum (not Float) — picking from
            // a list of well-known ratios is much friendlier than typing a
            // float, and the user explicitly asked for "比例(这个不支持自定义)".
            kind: OptionKind::Enum {
                flag: "+mat_letterbox_aspect_min",
                choices: &[
                    ("1.3333", "4:3 (1.33)"),
                    ("1.5",    "3:2 (1.50)"),
                    ("1.6",    "16:10 (1.60)"),
                    ("1.7778", "16:9 (1.78)"),
                    ("2.3333", "21:9 (2.33)"),
                    ("3.5556", "32:9 (3.56)"),
                ],
            },
            default_enabled: false,
            default_value: Some(OptionValue::Enum("1.7778".into())),
            label_zh: "画面比例",
            description_zh:
                "强制画面比例下限。配合 4:3 分辨率可移除黑边。仅支持常见比例预设，不支持自定义。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "fov_scale",
            category: "display",
            // Stored as degrees (70..=120) for human readability; the
            // emitted wire value is `degrees / 70` so 70°→1.0, 120°→~1.714.
            // The catalog id stays "fov_scale" so existing references in
            // the wider codebase still match.
            kind: OptionKind::FovDegrees {
                flag: "+cl_fovScale",
                min: 70,
                max: 120,
                base: 70,
            },
            default_enabled: false,
            default_value: Some(OptionValue::Int(120)),
            label_zh: "FOV 视野",
            description_zh:
                "视野角度。Apex 默认 70°，可调到 120°。常用预设：70 / 90 / 100 / 110 / 120。视野越大越广但可能晕 3D。启动项中会自动转换为 +cl_fovScale 缩放系数。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "wide_pillarbox",
            category: "display",
            kind: OptionKind::Toggle { args: &["+mat_wide_pillarbox", "0"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "超宽屏拉伸全屏",
            description_zh: "超宽屏显示器拉伸分辨率全屏。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "minimize_on_alt_tab",
            category: "display",
            kind: OptionKind::Toggle { args: &["+mat_minimize_on_alt_tab", "1"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "切屏最小化",
            description_zh: "类似 DX11 切屏让 apex 最小化。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "showpos",
            category: "display",
            kind: OptionKind::Toggle { args: &["+cl_showpos", "1"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "显示位置/角度/速度",
            description_zh: "在游戏中显示名称、位置、角度和速度。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "showfps",
            category: "display",
            kind: OptionKind::Toggle { args: &["+cl_showfps", "1"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "显示 FPS / 网络",
            description_zh: "显示性能与网络参数。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },

        // ===== 性能与帧率 =====
        OptionEntry {
            id: "fps_max",
            category: "performance",
            kind: OptionKind::Int { flag: "+fps_max", min: 0, max: 1000 },
            default_enabled: false,
            default_value: Some(OptionValue::Int(0)),
            label_zh: "锁定/解锁帧率",
            description_zh: "0 表示解锁；其他数字为上限（如 144）。",
            risk: RiskLevel::None,
            conflicts_with: &["no_render_on_input_thread"],
        },
        OptionEntry {
            id: "lobby_max_fps",
            category: "performance",
            kind: OptionKind::Int { flag: "+lobby_max_fps", min: 0, max: 1000 },
            default_enabled: false,
            default_value: Some(OptionValue::Int(0)),
            label_zh: "大厅帧率上限",
            description_zh: "0 表示解锁。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "high_priority",
            category: "performance",
            kind: OptionKind::Toggle { args: &["-high"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "高线程优先级",
            description_zh: "将游戏线程优先级设置为高。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "no_render_on_input_thread",
            category: "performance",
            kind: OptionKind::Toggle { args: &["-no_render_on_input_thread"], is_combo: false },
            default_enabled: false,
            default_value: None,
            label_zh: "降低输入延迟",
            description_zh: "提高帧数（CPU≥6 核 + 高回报率外设推荐）。",
            risk: RiskLevel::Caution,
            conflicts_with: &["fps_max"],
        },

        // ===== 操作与输入 =====
        OptionEntry {
            id: "mouse_optimize",
            category: "input",
            // Single combo replaces the four old switches: +m_rawinput,
            // -noforcemaccel, -noforcemspd, -noforcemparms. Mxtools groups
            // these as one entry too — flipping them individually doesn't
            // make sense for normal users.
            kind: OptionKind::Toggle {
                args: &[
                    "+m_rawinput",
                    "1",
                    "-noforcemaccel",
                    "-noforcemspd",
                    "-noforcemparms",
                ],
                is_combo: true,
            },
            default_enabled: false,
            default_value: None,
            label_zh: "优化鼠标输入（推荐）",
            description_zh:
                "一键开启 4 项鼠标优化：直接读取硬件信号、不强制系统鼠标加速 / 速度 / 参数。Apex 玩家强烈建议开启，可保证鼠标移动线性、不飘、不加速。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },

        // ===== 配音语言 =====
        OptionEntry {
            id: "miles_language",
            category: "voice",
            kind: OptionKind::Enum {
                flag: "+miles_language",
                choices: &[
                    ("mandarin", "普通话"),
                    ("english",  "英语"),
                    ("japanese", "日语（需额外语音包）"),
                    ("french",   "法语"),
                    ("german",   "德语"),
                    ("italian",  "意大利语"),
                    ("korean",   "韩语"),
                    ("polish",   "波兰语"),
                    ("russian",  "俄语"),
                    ("spanish",  "西班牙语"),
                ],
            },
            default_enabled: false,
            default_value: Some(OptionValue::Enum("english".into())),
            label_zh: "配音语言",
            description_zh: "更改游戏内角色配音（不影响 UI 语言）。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
        OptionEntry {
            id: "miles_channels",
            category: "voice",
            kind: OptionKind::Enum {
                flag: "+miles_channels",
                choices: &[
                    ("2", "立体声 (2)"),
                    ("8", "7.1 声道 (8)"),
                ],
            },
            default_enabled: false,
            default_value: Some(OptionValue::Enum("2".into())),
            label_zh: "声道数",
            description_zh: "音频输出声道。",
            risk: RiskLevel::None,
            conflicts_with: &[],
        },
    ],
});
