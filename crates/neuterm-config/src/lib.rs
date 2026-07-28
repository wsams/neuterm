//! YAML configuration for NeuTerm.
//!
//! See `specs/configuration.md` for the authoritative schema.

mod schema;
mod theme;

pub use schema::*;
pub use theme::*;

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{info, warn};

/// Errors loading or validating configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("invalid config: {0}")]
    Validate(String),
}

/// Resolve the user config path (does not create files).
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("NEUTERM_CONFIG") {
        return PathBuf::from(p);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("neuterm").join("config.yaml");
        }
        if let Some(home) = dirs::home_dir() {
            let xdg_style = home.join(".config").join("neuterm").join("config.yaml");
            if xdg_style.exists() {
                return xdg_style;
            }
            if let Some(app) = dirs::config_dir() {
                return app.join("neuterm").join("config.yaml");
            }
            return xdg_style;
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(dir) = dirs::config_dir() {
            return dir.join("neuterm").join("config.yaml");
        }
    }

    PathBuf::from("config.yaml")
}

/// Directory containing the shipped default config (relative to repo / install).
pub fn bundled_default_yaml() -> &'static str {
    include_str!("../../../configs/default.yaml")
}

/// Ensure the config directory exists and write a default file if missing.
pub fn ensure_user_config() -> Result<PathBuf, ConfigError> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    if !path.exists() {
        info!("writing default config to {}", path.display());
        fs::write(&path, bundled_default_yaml()).map_err(|source| ConfigError::Io {
            path: path.clone(),
            source,
        })?;
    }
    Ok(path)
}

/// Load config from an explicit path.
pub fn load_from_path(path: &Path) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut cfg: Config = serde_yaml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    cfg.validate()?;
    Ok(cfg)
}

/// Load user config, creating defaults on first run.
pub fn load() -> Result<(Config, PathBuf), ConfigError> {
    let path = ensure_user_config()?;
    let cfg = load_from_path(&path)?;
    Ok((cfg, path))
}

/// Load config or fall back to built-in defaults (never fails hard).
pub fn load_or_default() -> (Config, Option<PathBuf>) {
    match load() {
        Ok((cfg, path)) => (cfg, Some(path)),
        Err(err) => {
            warn!("config load failed ({err}); using built-in defaults");
            let mut cfg = Config::default();
            let _ = cfg.validate();
            (cfg, None)
        }
    }
}

impl Config {
    pub fn validate(&mut self) -> Result<(), ConfigError> {
        if self.window.initial_cols < 1 || self.window.initial_rows < 1 {
            return Err(ConfigError::Validate(
                "window.initial_cols and initial_rows must be >= 1".into(),
            ));
        }
        if self.font.size <= 0.0 {
            return Err(ConfigError::Validate("font.size must be > 0".into()));
        }
        if let Some(port) = self.ai.ollama.port {
            if port == 0 {
                return Err(ConfigError::Validate("ai.ollama.port must be 1–65535".into()));
            }
        }
        match &self.scrollback.lines {
            ScrollbackLines::Limited(n) if *n == 0 => {
                return Err(ConfigError::Validate(
                    "scrollback.lines must be > 0 or \"unlimited\"".into(),
                ));
            }
            _ => {}
        }
        if self.background.filter.blur < 0.0 {
            return Err(ConfigError::Validate(
                "background.filter.blur must be >= 0".into(),
            ));
        }
        Ok(())
    }

    /// Resolved visual theme. Builtin name + top-level color/padding overrides,
    /// or a fully inlined theme object.
    pub fn resolved_theme(&self) -> Theme {
        let mut theme = Theme::resolve(&self.theme);
        if matches!(self.theme, ThemeSpec::Name(_)) {
            // Legacy / simple configs: top-level colors & window.padding override the builtin.
            theme.colors = self.colors.clone();
            theme.window_padding = self.window.padding.clone();
            theme.panes.inactive_dim = self.panes.inactive_dim;
        }
        theme
    }

    /// Resolved Ollama base URL without trailing slash.
    pub fn ollama_base_url(&self) -> String {
        if let Some(url) = &self.ai.ollama.base_url {
            return url.trim_end_matches('/').to_string();
        }
        let host = self.ai.ollama.host.as_deref().unwrap_or("127.0.0.1");
        let port = self.ai.ollama.port.unwrap_or(11434);
        format!("http://{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_yaml_parses() {
        let mut cfg: Config = serde_yaml::from_str(bundled_default_yaml()).unwrap();
        cfg.validate().unwrap();
        assert!(cfg.window.initial_cols >= 1);
        assert_eq!(cfg.ollama_base_url(), "http://127.0.0.1:11434");
    }

    #[test]
    fn unlimited_scrollback() {
        let yaml = r#"
scrollback:
  lines: unlimited
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(cfg.scrollback.lines, ScrollbackLines::Unlimited));
    }

    #[test]
    fn theme_name_resolves() {
        let yaml = r#"
theme: midnight
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        let theme = cfg.resolved_theme();
        assert_eq!(theme.name, "midnight");
        assert_eq!(theme.tabs.gap, 6);
        assert_eq!(theme.tabs.bar_padding.top, theme.tabs.bar_padding.bottom);
        assert_eq!(theme.pane_inset.top, 0);
        assert_eq!(theme.pane_inset.left, theme.pane_inset.right);
    }

    #[test]
    fn theme_object_parses() {
        let yaml = r#"
theme:
  name: custom
  tabs:
    gap: 10
    bar_padding: { left: 4, right: 4, top: 4, bottom: 4 }
    tab_padding: { left: 8, right: 8, top: 8, bottom: 8 }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).unwrap();
        let theme = cfg.resolved_theme();
        assert_eq!(theme.name, "custom");
        assert_eq!(theme.tabs.gap, 10);
        assert_eq!(theme.tabs.tab_padding.left, 8);
    }
}
