use serde::{Deserialize, Deserializer, Serialize};

use crate::{Theme, ThemeSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Visual theme (`graphite`, `midnight`, or a full theme object).
    pub theme: ThemeSpec,
    pub window: WindowConfig,
    pub font: FontConfig,
    /// Terminal palette. When `theme` is a builtin name, these override the builtin colors.
    pub colors: ColorsConfig,
    pub background: BackgroundConfig,
    pub scrollback: ScrollbackConfig,
    pub bell: BellConfig,
    pub term: TermConfig,
    pub tabs: TabsConfig,
    pub panes: PanesConfig,
    pub keybindings: KeybindingsConfig,
    pub triggers: TriggersConfig,
    pub plugins: PluginsConfig,
    pub ai: AiConfig,
    pub performance: PerformanceConfig,
}

impl Default for Config {
    fn default() -> Self {
        // Construct field-wise — do not parse YAML here.
        // `#[serde(default)]` + Default-via-deserialize causes infinite recursion.
        let theme = Theme::graphite();
        Self {
            theme: ThemeSpec::Name(theme.name.clone()),
            window: WindowConfig {
                padding: theme.window_padding.clone(),
                ..WindowConfig::default()
            },
            font: FontConfig::default(),
            colors: theme.colors.clone(),
            background: BackgroundConfig::default(),
            scrollback: ScrollbackConfig::default(),
            bell: BellConfig::default(),
            term: TermConfig::default(),
            tabs: TabsConfig::default(),
            panes: PanesConfig {
                inactive_dim: theme.panes.inactive_dim,
            },
            keybindings: KeybindingsConfig::default(),
            triggers: TriggersConfig::default(),
            plugins: PluginsConfig::default(),
            ai: AiConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub initial_cols: u16,
    pub initial_rows: u16,
    pub padding: Padding,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            initial_cols: 120,
            initial_rows: 40,
            padding: Padding::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Padding {
    pub left: u32,
    pub right: u32,
    pub top: u32,
    pub bottom: u32,
}

impl Default for Padding {
    fn default() -> Self {
        Self {
            left: 8,
            right: 8,
            top: 8,
            bottom: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub ligatures: bool,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: "Adwaita Mono".into(),
            size: 14.0,
            ligatures: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorsConfig {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection_fg: String,
    pub selection_bg: String,
    pub ansi: Vec<String>,
    pub brights: Vec<String>,
    pub split: String,
}

impl Default for ColorsConfig {
    fn default() -> Self {
        Self {
            foreground: "#a5a7aa".into(),
            background: "#25282e".into(),
            cursor: "#a5a6aa".into(),
            selection_fg: "#000000".into(),
            selection_bg: "#7d7d7d".into(),
            ansi: vec![
                "#2e3239".into(),
                "#be861b".into(),
                "#2289b4".into(),
                "#d1b06e".into(),
                "#7d8fa4".into(),
                "#a25795".into(),
                "#5abfd5".into(),
                "#a5a6aa".into(),
            ],
            brights: vec![
                "#2e3239".into(),
                "#bd851b".into(),
                "#2289b4".into(),
                "#d0af6e".into(),
                "#7d8fa4".into(),
                "#a25794".into(),
                "#4ea7bb".into(),
                "#a5a6aa".into(),
            ],
            split: "#a5a7aa".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundConfig {
    pub image: Option<String>,
    pub overlay_color: String,
    pub overlay_opacity: f32,
    pub filter: BackgroundFilter,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            image: None,
            overlay_color: "#25282e".into(),
            overlay_opacity: 0.90,
            filter: BackgroundFilter::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundFilter {
    pub brightness: f32,
    pub blur: f32,
    pub saturation: f32,
}

impl Default for BackgroundFilter {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            blur: 0.0,
            saturation: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScrollbackConfig {
    pub lines: ScrollbackLines,
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self {
            lines: ScrollbackLines::Limited(200_000),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ScrollbackLines {
    Limited(usize),
    Unlimited,
}

impl Serialize for ScrollbackLines {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Limited(n) => serializer.serialize_u64(*n as u64),
            Self::Unlimited => serializer.serialize_str("unlimited"),
        }
    }
}

impl<'de> Deserialize<'de> for ScrollbackLines {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ScrollbackLines;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a positive integer or \"unlimited\"")
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(ScrollbackLines::Limited(v as usize))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                if v < 0 {
                    return Err(E::custom("scrollback lines must be positive"));
                }
                Ok(ScrollbackLines::Limited(v as usize))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.eq_ignore_ascii_case("unlimited") || v.eq_ignore_ascii_case("infinity") {
                    Ok(ScrollbackLines::Unlimited)
                } else if let Ok(n) = v.parse::<usize>() {
                    Ok(ScrollbackLines::Limited(n))
                } else {
                    Err(E::custom(format!("invalid scrollback.lines: {v}")))
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BellMode {
    None,
    Visual,
    Audible,
    Both,
    Pulse,
}

impl Default for BellMode {
    fn default() -> Self {
        Self::Pulse
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BellConfig {
    pub mode: BellMode,
    pub audible: bool,
}

impl Default for BellConfig {
    fn default() -> Self {
        Self {
            mode: BellMode::Pulse,
            audible: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TermConfig {
    pub program: String,
    pub shell: Option<String>,
    pub cwd: Option<String>,
}

impl Default for TermConfig {
    fn default() -> Self {
        Self {
            program: "xterm-256color".into(),
            shell: None,
            cwd: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TabsConfig {
    pub show_bar: bool,
    pub hide_if_only_one: bool,
    pub default_title_mode: TitleMode,
}

impl Default for TabsConfig {
    fn default() -> Self {
        Self {
            show_bar: true,
            hide_if_only_one: false,
            default_title_mode: TitleMode::Process,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TitleMode {
    Process,
    Static,
}

impl Default for TitleMode {
    fn default() -> Self {
        Self::Process
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PanesConfig {
    pub inactive_dim: f32,
}

impl Default for PanesConfig {
    fn default() -> Self {
        Self { inactive_dim: 0.15 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub bindings: Vec<KeyBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    #[serde(default)]
    pub mods: Vec<String>,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TriggersConfig {
    pub enabled: bool,
    pub rules: Vec<TriggerRule>,
}

impl Default for TriggersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    pub name: String,
    #[serde(rename = "match")]
    pub pattern: String,
    #[serde(default = "default_match_type")]
    pub match_type: MatchType,
    #[serde(default = "default_scope")]
    pub scope: TriggerScope,
    pub style: TriggerStyle,
}

fn default_match_type() -> MatchType {
    MatchType::Regex
}

fn default_scope() -> TriggerScope {
    TriggerScope::Line
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    Regex,
    String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TriggerScope {
    Line,
    Match,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerStyle {
    pub foreground: Option<String>,
    pub background: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub underline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginsConfig {
    pub enabled: bool,
    pub dirs: Vec<String>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub enabled: bool,
    pub ollama: OllamaConfig,
    pub command_help: CommandHelpConfig,
    pub chat: ChatConfig,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ollama: OllamaConfig::default(),
            command_help: CommandHelpConfig::default(),
            chat: ChatConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub base_url: Option<String>,
    pub model: String,
    pub timeout_ms: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: Some("127.0.0.1".into()),
            port: Some(11434),
            base_url: None,
            model: "llama3.2".into(),
            timeout_ms: 60_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandHelpConfig {
    pub shortcut: String,
    pub system_prompt: Option<String>,
}

impl Default for CommandHelpConfig {
    fn default() -> Self {
        Self {
            shortcut: "default".into(),
            system_prompt: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatConfig {
    pub title: String,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            title: "AI Chat".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RendererMode {
    Auto,
    Gpu,
    Cpu,
}

impl Default for RendererMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceConfig {
    pub renderer: RendererMode,
    pub vsync: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            renderer: RendererMode::Auto,
            vsync: true,
        }
    }
}
