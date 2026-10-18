use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde_json::Value;

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
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

// Reserved format group. Parsed and stored, never consumed in V1.
// The formatter epic validates rule keys later.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatConfig {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub rules: HashMap<String, String>,
}

// Grouped `fls.json` file: lint plus reserved format.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlsConfig {
    pub lint: LintConfig,
    pub format: FormatConfig,
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

    // Parse lint JSON via untyped traversal. Accepts a lint object or a
    // full `fls.json` document (uses its `lint` group when present).
    pub fn parse_json(text: &str) -> Result<Self, LintConfigError> {
        let value: Value =
            serde_json::from_str(text).map_err(|err| LintConfigError::Parse(err.to_string()))?;
        let target = match value.get("lint") {
            Some(group) if group.is_object() => group,
            _ => &value,
        };
        parse_lint_value(target)
    }
}

impl FlsConfig {
    // Parse grouped `fls.json` via untyped traversal.
    pub fn parse_json(text: &str) -> Result<Self, LintConfigError> {
        let value: Value =
            serde_json::from_str(text).map_err(|err| LintConfigError::Parse(err.to_string()))?;
        let table = value
            .as_object()
            .ok_or_else(|| LintConfigError::Parse("fls.json must be an object".to_string()))?;
        let mut out = Self::default();
        if let Some(lint) = table.get("lint") {
            out.lint = parse_lint_value(lint)?;
        }
        if let Some(format) = table.get("format") {
            out.format = parse_format_value(format)?;
        }
        Ok(out)
    }
}

fn string_list(value: &Value, group: &str, key: &str) -> Result<Vec<String>, LintConfigError> {
    let Some(list) = value.get(key) else {
        return Ok(Vec::new());
    };
    let items = list
        .as_array()
        .ok_or_else(|| LintConfigError::Parse(format!("{group} {key} must be an array")))?;
    let mut out = Vec::new();
    for entry in items {
        let name = entry
            .as_str()
            .ok_or_else(|| LintConfigError::Parse(format!("{group} {key} must be strings")))?;
        out.push(name.to_string());
    }
    Ok(out)
}

fn parse_lint_value(value: &Value) -> Result<LintConfig, LintConfigError> {
    let table = value
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("lint must be an object".to_string()))?;
    let mut out = LintConfig {
        plugins: string_list(value, "lint", "plugins")?,
        includes: string_list(value, "lint", "includes")?,
        excludes: string_list(value, "lint", "excludes")?,
        ..Default::default()
    };
    let Some(rules) = table.get("rules") else {
        return Ok(out);
    };
    let rules = rules
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("lint rules must be an object".to_string()))?;
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
    Ok(out)
}

fn parse_format_value(value: &Value) -> Result<FormatConfig, LintConfigError> {
    if !value.is_object() {
        return Err(LintConfigError::Parse(
            "format must be an object".to_string(),
        ));
    }
    let mut out = FormatConfig {
        includes: string_list(value, "format", "includes")?,
        excludes: string_list(value, "format", "excludes")?,
        ..Default::default()
    };
    let Some(rules) = value.get("rules") else {
        return Ok(out);
    };
    let rules = rules
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("format rules must be an object".to_string()))?;
    for (key, val) in rules {
        let raw = val
            .as_str()
            .ok_or_else(|| LintConfigError::Parse(format!("format rule {key} must be a string")))?;
        out.rules.insert(key.clone(), raw.to_string());
    }
    Ok(out)
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

    #[test]
    fn json_parses_grouped_rules_and_paths() {
        let text = r#"{"lint":{"includes":["**/*.sol"],"excludes":["lib/**"],"rules":{"correctness/unused":"warn"}}}"#;
        let config = LintConfig::parse_json(text).expect("parses");
        assert_eq!(config.includes, vec!["**/*.sol".to_string()]);
        assert_eq!(config.excludes, vec!["lib/**".to_string()]);
        assert_eq!(config.level_for(RuleId::UNUSED), Some(RuleLevel::Warn));
    }

    #[test]
    fn json_missing_groups_give_defaults() {
        let config = LintConfig::parse_json("{}").expect("parses");
        assert!(config.includes.is_empty());
        assert!(config.excludes.is_empty());
        assert!(config.rules.is_empty());
        let full = FlsConfig::parse_json("{}").expect("parses");
        assert_eq!(full, FlsConfig::default());
    }

    #[test]
    fn json_unknown_rule_fails() {
        let text = r#"{"lint":{"rules":{"correctness/nope":"warn"}}}"#;
        let err = LintConfig::parse_json(text).expect_err("unknown rule fails");
        assert_eq!(
            err,
            LintConfigError::UnknownRule("correctness/nope".to_string())
        );
    }

    #[test]
    fn json_bad_level_fails() {
        let text = r#"{"lint":{"rules":{"correctness/unused":"sometimes"}}}"#;
        let err = LintConfig::parse_json(text).expect_err("bad level fails");
        assert!(matches!(err, LintConfigError::InvalidLevel(_)));
    }

    #[test]
    fn json_format_group_stores_raw_without_validation() {
        let text = r#"{"format":{"includes":[],"excludes":[],"rules":{"future/rule":"terse"}}}"#;
        let full = FlsConfig::parse_json(text).expect("parses");
        assert_eq!(
            full.format.rules.get("future/rule"),
            Some(&"terse".to_string())
        );
    }

    #[test]
    fn json_bad_shape_is_parse_error() {
        let err = FlsConfig::parse_json(r#"{"lint":[]}"#).expect_err("bad shape fails");
        assert!(matches!(err, LintConfigError::Parse(_)));
        let err = FlsConfig::parse_json("not json").expect_err("invalid json fails");
        assert!(matches!(err, LintConfigError::Parse(_)));
    }

    #[test]
    fn style_batch_two_rule_parses_off_warn_err() {
        let text = "[lint.rules]\n\"style/blank-lines\" = \"off\"\n\"style/natspec-missing\" = \"warn\"\n\"style/line-length\" = \"err\"\n";
        let config = LintConfig::parse_str(text).expect("parses");
        assert_eq!(config.level_for(RuleId::BLANK_LINES), Some(RuleLevel::Off));
        assert_eq!(
            config.level_for(RuleId::NATSPEC_MISSING),
            Some(RuleLevel::Warn)
        );
        assert_eq!(config.level_for(RuleId::LINE_LENGTH), Some(RuleLevel::Err));
        assert_eq!(RuleId::BLANK_LINES.default_severity(), Severity::Warning);
        assert_eq!(
            RuleId::NATSPEC_MISSING.default_severity(),
            Severity::Warning
        );
        assert_eq!(RuleId::NATSPEC_PARAM.default_severity(), Severity::Warning);
        assert_eq!(RuleId::LINE_LENGTH.default_severity(), Severity::Warning);
    }
}
