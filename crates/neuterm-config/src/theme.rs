//! Swappable visual themes.
//!
//! A [`Theme`] is a single data object: colors, chrome metrics, and padding.
//! Swap the theme → new look. Built-ins live in [`Theme::builtin`].

use serde::{Deserialize, Serialize};

use crate::{ColorsConfig, Padding};

/// Complete visual theme for NeuTerm chrome + terminal colors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub colors: ColorsConfig,
    /// Padding between the pane focus border and the cell grid.
    pub window_padding: Padding,
    /// Margin around the pane chrome (outside the focus border).
    /// Top is usually `0` so the tab bar sits flush on the content frame.
    pub pane_inset: Padding,
    pub tabs: TabChrome,
    pub panes: PaneChrome,
}

impl Default for Theme {
    fn default() -> Self {
        Self::graphite()
    }
}

impl Theme {
    /// Built-in themes. Unknown names fall back to graphite.
    pub fn builtin(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "graphite" | "default" => Self::graphite(),
            "midnight" => Self::midnight(),
            other => {
                let mut t = Self::graphite();
                t.name = other.to_string();
                t
            }
        }
    }

    /// Resolve from config theme block: start from builtin name, apply overrides.
    pub fn resolve(spec: &ThemeSpec) -> Self {
        match spec {
            ThemeSpec::Name(name) => Self::builtin(name),
            ThemeSpec::Full(theme) => {
                let mut base = Self::builtin(&theme.name);
                // Full theme replaces fields that were present via serde defaults
                // on nested structs — use the provided theme wholesale when name
                // matches a custom, otherwise merge chrome/colors from `theme`.
                base.name = theme.name.clone();
                base.colors = theme.colors.clone();
                base.window_padding = theme.window_padding.clone();
                base.pane_inset = theme.pane_inset.clone();
                base.tabs = theme.tabs.clone();
                base.panes = theme.panes.clone();
                base
            }
        }
    }

    pub fn graphite() -> Self {
        Self {
            name: "graphite".into(),
            colors: ColorsConfig::default(),
            window_padding: Padding::uniform(8),
            // Flush under the tab bar; even inset on the other three sides.
            pane_inset: Padding {
                left: 4,
                right: 4,
                top: 0,
                bottom: 4,
            },
            tabs: TabChrome::graphite(),
            panes: PaneChrome::default(),
        }
    }

    pub fn midnight() -> Self {
        Self {
            name: "midnight".into(),
            colors: ColorsConfig {
                foreground: "#c8ccd4".into(),
                background: "#0f1115".into(),
                cursor: "#c8ccd4".into(),
                selection_fg: "#0f1115".into(),
                selection_bg: "#3d4f65".into(),
                ansi: vec![
                    "#1a1d23".into(),
                    "#e06c75".into(),
                    "#98c379".into(),
                    "#e5c07b".into(),
                    "#61afef".into(),
                    "#c678dd".into(),
                    "#56b6c2".into(),
                    "#c8ccd4".into(),
                ],
                brights: vec![
                    "#4b5263".into(),
                    "#e06c75".into(),
                    "#98c379".into(),
                    "#e5c07b".into(),
                    "#61afef".into(),
                    "#c678dd".into(),
                    "#56b6c2".into(),
                    "#ffffff".into(),
                ],
                split: "#2a2f3a".into(),
            },
            window_padding: Padding::uniform(10),
            pane_inset: Padding {
                left: 4,
                right: 4,
                top: 0,
                bottom: 4,
            },
            tabs: TabChrome {
                bar_background: "#0b0d10".into(),
                active_background: "#1a1f2a".into(),
                inactive_background: "#0b0d10".into(),
                active_foreground: "#c8ccd4".into(),
                inactive_foreground: "#6b7280".into(),
                separator_color: "#2a2f3a".into(),
                separator_height: 1,
                bar_padding: Padding::uniform(6),
                tab_padding: Padding::uniform(6),
                gap: 6,
            },
            panes: PaneChrome {
                inactive_dim: 0.18,
                focus_border: "#61afef".into(),
            },
        }
    }
}

/// Config form: either a builtin name (`theme: graphite`) or a full object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ThemeSpec {
    Name(String),
    Full(Theme),
}

impl Default for ThemeSpec {
    fn default() -> Self {
        Self::Name("graphite".into())
    }
}

/// Tab bar chrome metrics and colors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TabChrome {
    pub bar_background: String,
    pub active_background: String,
    pub inactive_background: String,
    pub active_foreground: String,
    pub inactive_foreground: String,
    pub separator_color: String,
    pub separator_height: u32,
    /// Padding around the row of tabs inside the bar.
    pub bar_padding: Padding,
    /// Padding inside each tab chip (and matching + button).
    pub tab_padding: Padding,
    /// Horizontal gap between tab chips / + button.
    pub gap: u32,
}

impl Default for TabChrome {
    fn default() -> Self {
        Self::graphite()
    }
}

impl TabChrome {
    pub fn graphite() -> Self {
        Self {
            bar_background: "#1c1f24".into(),
            active_background: "#2e3239".into(),
            inactive_background: "#1c1f24".into(),
            active_foreground: "#a5a7aa".into(),
            inactive_foreground: "#6a6e76".into(),
            separator_color: "#3a3f48".into(),
            separator_height: 1,
            // Even outer padding around the tab row.
            bar_padding: Padding::uniform(6),
            // Even inner padding around tab label / + glyph.
            tab_padding: Padding::uniform(6),
            gap: 6,
        }
    }

    /// Total tab bar height for a given cell height.
    pub fn bar_height(&self, cell_h: u32) -> u32 {
        let tab_h = cell_h + self.tab_padding.top + self.tab_padding.bottom;
        self.bar_padding.top + tab_h + self.bar_padding.bottom + self.separator_height
    }

    /// Height of a tab chip / + button.
    pub fn tab_height(&self, cell_h: u32) -> u32 {
        cell_h + self.tab_padding.top + self.tab_padding.bottom
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneChrome {
    pub inactive_dim: f32,
    pub focus_border: String,
}

impl Default for PaneChrome {
    fn default() -> Self {
        Self {
            inactive_dim: 0.15,
            focus_border: "#a5a6aa".into(),
        }
    }
}

impl Padding {
    pub fn uniform(v: u32) -> Self {
        Self {
            left: v,
            right: v,
            top: v,
            bottom: v,
        }
    }
}
