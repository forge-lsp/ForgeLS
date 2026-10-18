use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use super::{RuleId, Severity};

// Per-rule level from `[lint.rules]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleLevel {
    Off,
    Warn,
    Err,
}

impl RuleLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Warn => "warn",
            Self::Err => "err",
        }
    }

    // Off maps to nothing: the runner drops the diagnostic.
    pub fn severity(self) -> Option<Severity> {
        match self {
            Self::Off => None,
            Self::Warn => Some(Severity::Warning),
            Self::Err => Some(Severity::Error),
        }
    }
}

impl FromStr for RuleLevel {
    type Err = ();

    fn from_str(raw: &str) -> Result<Self, ()> {
        match raw {
            "off" => Ok(Self::Off),
            "warn" => Ok(Self::Warn),
            "err" => Ok(Self::Err),
            _ => Err(()),
        }
    }
}

// Parsed `[lint]` section. No file discovery: callers load the text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LintConfig {
    pub plugins: Vec<String>,
    pub rules: HashMap<String, RuleLevel>,
}

impl LintConfig {
    pub fn level_for(&self, rule: RuleId) -> Option<RuleLevel> {
        self.rules.get(rule.as_str()).copied()
    }

    // Parse `[lint]` plus `[lint.rules]` from TOML text.
    // Unknown rule IDs fail fast so typos never pass silent.
    pub fn parse_str(text: &str) -> Result<Self, LintConfigError> {
        let table: toml::Table = text
            .parse()
            .map_err(|err: toml::de::Error| LintConfigError::Parse(err.to_string()))?;
        let Some(lint) = table.get("lint") else {
            return Ok(Self::default());
        };
        let lint = lint
            .as_table()
            .ok_or_else(|| LintConfigError::Parse("[lint] must be a table".to_string()))?;
        let mut out = Self::default();
        if let Some(plugins) = lint.get("plugins") {
            let list = plugins.as_array().ok_or_else(|| {
                LintConfigError::Parse("[lint] plugins must be an array".to_string())
            })?;
            for entry in list {
                let name = entry.as_str().ok_or_else(|| {
                    LintConfigError::Parse("[lint] plugins must be strings".to_string())
                })?;
                out.plugins.push(name.to_string());
            }
        }
        if let Some(rules) = lint.get("rules") {
            let rules = rules.as_table().ok_or_else(|| {
                LintConfigError::Parse("[lint.rules] must be a table".to_string())
            })?;
            for (id, level) in rules {
                if !RuleId::all().iter().any(|rule| rule.as_str() == id) {
                    return Err(LintConfigError::UnknownRule(id.clone()));
                }
                let raw = level.as_str().ok_or_else(|| {
                    LintConfigError::InvalidLevel(format!("{id} must be off, warn, or err"))
                })?;
                let parsed = raw
                    .parse()
                    .map_err(|_: ()| LintConfigError::InvalidLevel(format!("{id} has {raw}")))?;
                out.rules.insert(id.clone(), parsed);
            }
        }
        Ok(out)
    }
}

// Config failure: bad TOML, unknown rule ID, or bad level string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintConfigError {
    Parse(String),
    UnknownRule(String),
    InvalidLevel(String),
}

impl fmt::Display for LintConfigError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(detail) => write!(out, "invalid lint config: {detail}"),
            Self::UnknownRule(id) => write!(out, "unknown lint rule: {id}"),
            Self::InvalidLevel(detail) => write!(out, "invalid lint level: {detail}"),
        }
    }
}

impl std::error::Error for LintConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_lint_section_gives_default() {
        let config = LintConfig::parse_str("").expect("empty parses");
        assert!(config.plugins.is_empty());
        assert!(config.rules.is_empty());
    }

    #[test]
    fn parses_plugins_and_rule_levels() {
        let text = "[lint]\nplugins = [\"correctness\"]\n[lint.rules]\n\"correctness/unused\" = \"warn\"\n\"correctness/parse-error\" = \"off\"\n";
        let config = LintConfig::parse_str(text).expect("parses");
        assert_eq!(config.plugins, vec!["correctness".to_string()]);
        assert_eq!(config.level_for(RuleId::UNUSED), Some(RuleLevel::Warn));
        assert_eq!(config.level_for(RuleId::PARSE_ERROR), Some(RuleLevel::Off));
        assert_eq!(config.level_for(RuleId::OVERRIDE), None);
    }

    #[test]
    fn unknown_rule_id_fails_fast() {
        let text = "[lint.rules]\n\"correctness/nope\" = \"warn\"\n";
        let err = LintConfig::parse_str(text).expect_err("unknown rule fails");
        assert_eq!(
            err,
            LintConfigError::UnknownRule("correctness/nope".to_string())
        );
    }

    #[test]
    fn bad_level_string_fails() {
        let text = "[lint.rules]\n\"correctness/unused\" = \"sometimes\"\n";
        let err = LintConfig::parse_str(text).expect_err("bad level fails");
        assert!(matches!(err, LintConfigError::InvalidLevel(_)));
    }

    #[test]
    fn off_level_maps_to_no_severity() {
        assert_eq!(RuleLevel::Off.severity(), None);
        assert_eq!(RuleLevel::Warn.severity(), Some(Severity::Warning));
        assert_eq!(RuleLevel::Err.severity(), Some(Severity::Error));
    }
}
