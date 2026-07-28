//! Output triggers — style transforms for matching lines.
//!
//! See `specs/plugins-triggers.md`.

use neuterm_config::{MatchType, TriggerRule, TriggerScope, TriggersConfig};
use regex::Regex;
use tracing::warn;

#[derive(Debug, Clone, Copy)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone)]
pub struct AppliedStyle {
    pub foreground: Option<Rgba>,
    pub background: Option<Rgba>,
    pub bold: bool,
    pub underline: bool,
}

#[derive(Debug, Clone)]
pub struct LineDecoration {
    /// Inclusive start column, exclusive end. For `scope: line`, covers full line.
    pub start: usize,
    pub end: usize,
    pub style: AppliedStyle,
}

struct CompiledRule {
    #[allow(dead_code)]
    name: String,
    regex: Option<Regex>,
    literal: Option<String>,
    scope: TriggerScope,
    style: AppliedStyle,
}

pub struct TriggerEngine {
    enabled: bool,
    rules: Vec<CompiledRule>,
}

impl TriggerEngine {
    pub fn from_config(cfg: &TriggersConfig) -> Self {
        let mut rules = Vec::new();
        for rule in &cfg.rules {
            match compile_rule(rule) {
                Ok(r) => rules.push(r),
                Err(err) => warn!("skipping trigger '{}': {err}", rule.name),
            }
        }
        Self {
            enabled: cfg.enabled,
            rules,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn reload(&mut self, cfg: &TriggersConfig) {
        *self = Self::from_config(cfg);
    }

    /// Decorations for a single line of text.
    pub fn decorations_for_line(&self, line: &str) -> Vec<LineDecoration> {
        if !self.enabled {
            return Vec::new();
        }
        let mut out = Vec::new();
        for rule in &self.rules {
            let matched = if let Some(re) = &rule.regex {
                re.is_match(line)
            } else if let Some(lit) = &rule.literal {
                line.contains(lit)
            } else {
                false
            };
            if !matched {
                continue;
            }

            match rule.scope {
                TriggerScope::Line => {
                    out.push(LineDecoration {
                        start: 0,
                        end: line.chars().count().max(1),
                        style: rule.style.clone(),
                    });
                }
                TriggerScope::Match => {
                    if let Some(re) = &rule.regex {
                        for m in re.find_iter(line) {
                            let start = line[..m.start()].chars().count();
                            let end = start + m.as_str().chars().count();
                            out.push(LineDecoration {
                                start,
                                end,
                                style: rule.style.clone(),
                            });
                        }
                    } else if let Some(lit) = &rule.literal {
                        let mut search_start = 0;
                        while let Some(pos) = line[search_start..].find(lit) {
                            let abs = search_start + pos;
                            let start = line[..abs].chars().count();
                            let end = start + lit.chars().count();
                            out.push(LineDecoration {
                                start,
                                end,
                                style: rule.style.clone(),
                            });
                            search_start = abs + lit.len();
                        }
                    }
                }
            }
        }
        out
    }
}

fn compile_rule(rule: &TriggerRule) -> Result<CompiledRule, String> {
    let style = AppliedStyle {
        foreground: rule.style.foreground.as_deref().and_then(parse_hex_color),
        background: rule.style.background.as_deref().and_then(parse_hex_color),
        bold: rule.style.bold,
        underline: rule.style.underline,
    };
    match rule.match_type {
        MatchType::Regex => {
            let regex = Regex::new(&rule.pattern).map_err(|e| e.to_string())?;
            Ok(CompiledRule {
                name: rule.name.clone(),
                regex: Some(regex),
                literal: None,
                scope: rule.scope.clone(),
                style,
            })
        }
        MatchType::String => Ok(CompiledRule {
            name: rule.name.clone(),
            regex: None,
            literal: Some(rule.pattern.clone()),
            scope: rule.scope.clone(),
            style,
        }),
    }
}

pub fn parse_hex_color(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        6 => {
            let n = u32::from_str_radix(s, 16).ok()?;
            Some(Rgba {
                r: ((n >> 16) & 0xff) as u8,
                g: ((n >> 8) & 0xff) as u8,
                b: (n & 0xff) as u8,
                a: 255,
            })
        }
        8 => {
            let n = u32::from_str_radix(s, 16).ok()?;
            Some(Rgba {
                r: ((n >> 24) & 0xff) as u8,
                g: ((n >> 16) & 0xff) as u8,
                b: ((n >> 8) & 0xff) as u8,
                a: (n & 0xff) as u8,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuterm_config::{TriggerStyle, TriggersConfig};

    #[test]
    fn production_line_highlight() {
        let cfg = TriggersConfig {
            enabled: true,
            rules: vec![TriggerRule {
                name: "prod".into(),
                pattern: r"(?i)\b(production|prod|prd|critical)\b".into(),
                match_type: MatchType::Regex,
                scope: TriggerScope::Line,
                style: TriggerStyle {
                    foreground: Some("#ffffff".into()),
                    background: Some("#c0392b".into()),
                    bold: false,
                    underline: false,
                },
            }],
        };
        let engine = TriggerEngine::from_config(&cfg);
        let decs = engine.decorations_for_line("deploying to production now");
        assert_eq!(decs.len(), 1);
        assert_eq!(decs[0].start, 0);
        assert!(decs[0].style.background.is_some());
    }
}
