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
    pub external: Vec<ExternalRef>,
}

// One external WASM plugin reference from `lint.external`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalRef {
    pub path: String,
}

// Typed format group. `line_length`, `tab_width`, and `style` feed
// the formatter; `quote_style` and `bracket_spacing` are parsed and
// stored but never consumed (follow-up: quote and bracket rules).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FormatConfig {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub rules: HashMap<String, String>,
    pub line_length: Option<usize>,
    pub tab_width: Option<usize>,
    pub style: Option<String>,
    pub quote_style: Option<String>,
    pub bracket_spacing: Option<String>,
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
    // `lint.external` is JSON-only in V1: TOML use is a parse error
    // that points to fls.json.
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
        if lint.contains_key("external") {
            return Err(LintConfigError::Parse(
                "lint external requires fls.json (JSON-only V1)".to_string(),
            ));
        }
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
                validate_rule_id(id, &[])?;
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
        Self::parse_json_with_known(text, &[])
    }

    // Parse lint JSON with known guest IDs. Builtins always pass;
    // guest IDs pass when listed in `guest_ids` and well formed
    // (non-empty, no whitespace); anything else is UnknownRule.
    pub fn parse_json_with_known(
        text: &str,
        guest_ids: &[String],
    ) -> Result<Self, LintConfigError> {
        let value: Value =
            serde_json::from_str(text).map_err(|err| LintConfigError::Parse(err.to_string()))?;
        let target = match value.get("lint") {
            Some(group) if group.is_object() => group,
            _ => &value,
        };
        parse_lint_value_with(target, guest_ids)
    }
}

impl FlsConfig {
    // Parse grouped `fls.json` via untyped traversal.
    pub fn parse_json(text: &str) -> Result<Self, LintConfigError> {
        Self::parse_json_with_known(text, &[])
    }

    // Parse grouped `fls.json` with known guest IDs for rule validation.
    pub fn parse_json_with_known(
        text: &str,
        guest_ids: &[String],
    ) -> Result<Self, LintConfigError> {
        let value: Value =
            serde_json::from_str(text).map_err(|err| LintConfigError::Parse(err.to_string()))?;
        let table = value
            .as_object()
            .ok_or_else(|| LintConfigError::Parse("fls.json must be an object".to_string()))?;
        let mut out = Self::default();
        if let Some(lint) = table.get("lint") {
            out.lint = parse_lint_value_with(lint, guest_ids)?;
        }
        if let Some(format) = table.get("format") {
            out.format = parse_format_value(format)?;
        }
        Ok(out)
    }

    // Extract `lint.external` paths without validating rules. Used at
    // startup to load guests before the full rule check runs.
    pub fn parse_external_refs(text: &str) -> Result<Vec<ExternalRef>, LintConfigError> {
        let value: Value =
            serde_json::from_str(text).map_err(|err| LintConfigError::Parse(err.to_string()))?;
        let Some(lint) = value.get("lint") else {
            return Ok(Vec::new());
        };
        if !lint.is_object() {
            return Err(LintConfigError::Parse("lint must be an object".to_string()));
        }
        parse_external_value(lint)
    }
}

// Validate one rule ID: builtin always passes, guest passes when listed
// in `guest_ids` and well formed, else UnknownRule fail-fast.
pub fn validate_rule_id(id: &str, guest_ids: &[String]) -> Result<(), LintConfigError> {
    if RuleId::all().iter().any(|rule| rule.as_str() == id) {
        return Ok(());
    }
    if guest_ids.iter().any(|known| known == id) && RuleId::is_guest_id(id) {
        return Ok(());
    }
    Err(LintConfigError::UnknownRule(id.to_string()))
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

fn parse_external_value(value: &Value) -> Result<Vec<ExternalRef>, LintConfigError> {
    let Some(raw) = value.get("external") else {
        return Ok(Vec::new());
    };
    let list = raw
        .as_array()
        .ok_or_else(|| LintConfigError::Parse("lint external must be an array".to_string()))?;
    let mut out = Vec::new();
    for entry in list {
        let table = entry
            .as_object()
            .ok_or_else(|| LintConfigError::Parse("lint external must be objects".to_string()))?;
        let path = table.get("path").and_then(Value::as_str).ok_or_else(|| {
            LintConfigError::Parse("lint external path must be a string".to_string())
        })?;
        if path.is_empty() {
            return Err(LintConfigError::Parse(
                "lint external path must not be empty".to_string(),
            ));
        }
        out.push(ExternalRef {
            path: path.to_string(),
        });
    }
    Ok(out)
}

fn parse_lint_value_with(
    value: &Value,
    guest_ids: &[String],
) -> Result<LintConfig, LintConfigError> {
    let table = value
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("lint must be an object".to_string()))?;
    let mut out = LintConfig {
        plugins: string_list(value, "lint", "plugins")?,
        includes: string_list(value, "lint", "includes")?,
        excludes: string_list(value, "lint", "excludes")?,
        external: parse_external_value(value)?,
        ..Default::default()
    };
    let Some(rules) = table.get("rules") else {
        return Ok(out);
    };
    let rules = rules
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("lint rules must be an object".to_string()))?;
    for (id, level) in rules {
        validate_rule_id(id, guest_ids)?;
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
    let table = value
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("format must be an object".to_string()))?;
    const KNOWN: &[&str] = &[
        "includes",
        "excludes",
        "rules",
        "line_length",
        "tab_width",
        "style",
        "quote_style",
        "bracket_spacing",
    ];
    for key in table.keys() {
        if !KNOWN.contains(&key.as_str()) {
            return Err(LintConfigError::Parse(format!("unknown format key: {key}")));
        }
    }
    let mut out = FormatConfig {
        includes: string_list(value, "format", "includes")?,
        excludes: string_list(value, "format", "excludes")?,
        ..Default::default()
    };
    if let Some(raw) = table.get("line_length") {
        out.line_length = Some(parse_format_usize(raw, "line_length")?);
        if out.line_length == Some(0) {
            return Err(LintConfigError::Parse(
                "format line_length must be above 0".to_string(),
            ));
        }
    }
    if let Some(raw) = table.get("tab_width") {
        let width = parse_format_usize(raw, "tab_width")?;
        if width == 0 || width > 8 {
            return Err(LintConfigError::Parse(
                "format tab_width must be 1 to 8".to_string(),
            ));
        }
        out.tab_width = Some(width);
    }
    if let Some(raw) = table.get("style") {
        let name = raw
            .as_str()
            .ok_or_else(|| LintConfigError::Parse("format style must be a string".to_string()))?;
        if name != "space" && name != "tab" {
            return Err(LintConfigError::Parse(
                "format style must be space or tab".to_string(),
            ));
        }
        out.style = Some(name.to_string());
    }
    if let Some(raw) = table.get("quote_style") {
        let name = raw.as_str().ok_or_else(|| {
            LintConfigError::Parse("format quote_style must be a string".to_string())
        })?;
        out.quote_style = Some(name.to_string());
    }
    if let Some(raw) = table.get("bracket_spacing") {
        let name = raw.as_str().ok_or_else(|| {
            LintConfigError::Parse("format bracket_spacing must be a string".to_string())
        })?;
        out.bracket_spacing = Some(name.to_string());
    }
    let Some(rules) = value.get("rules") else {
        return Ok(out);
    };
    let rules = rules
        .as_object()
        .ok_or_else(|| LintConfigError::Parse("format rules must be an object".to_string()))?;
    for (key, val) in rules {
        if ![
            "line_length",
            "tab_width",
            "style",
            "quote_style",
            "bracket_spacing",
        ]
        .contains(&key.as_str())
        {
            return Err(LintConfigError::UnknownRule(key.clone()));
        }
        let raw = val
            .as_str()
            .ok_or_else(|| LintConfigError::Parse(format!("format rule {key} must be a string")))?;
        match key.as_str() {
            "line_length" => {
                let parsed: usize = raw.parse().map_err(|_| {
                    LintConfigError::Parse(format!("format rule {key} must be a number"))
                })?;
                if parsed == 0 {
                    return Err(LintConfigError::Parse(
                        "format line_length must be above 0".to_string(),
                    ));
                }
                if out.line_length.is_none() {
                    out.line_length = Some(parsed);
                }
            }
            "tab_width" => {
                let parsed: usize = raw.parse().map_err(|_| {
                    LintConfigError::Parse(format!("format rule {key} must be a number"))
                })?;
                if parsed == 0 || parsed > 8 {
                    return Err(LintConfigError::Parse(
                        "format tab_width must be 1 to 8".to_string(),
                    ));
                }
                if out.tab_width.is_none() {
                    out.tab_width = Some(parsed);
                }
            }
            "style" => {
                if raw != "space" && raw != "tab" {
                    return Err(LintConfigError::Parse(
                        "format style must be space or tab".to_string(),
                    ));
                }
                if out.style.is_none() {
                    out.style = Some(raw.to_string());
                }
            }
            "quote_style" | "bracket_spacing" => {
                if out.quote_style.is_none() && key == "quote_style" {
                    out.quote_style = Some(raw.to_string());
                }
                if out.bracket_spacing.is_none() && key == "bracket_spacing" {
                    out.bracket_spacing = Some(raw.to_string());
                }
            }
            _ => {}
        }
        out.rules.insert(key.clone(), raw.to_string());
    }
    Ok(out)
}

fn parse_format_usize(raw: &Value, key: &str) -> Result<usize, LintConfigError> {
    if let Some(num) = raw.as_u64() {
        let value = num as usize;
        if num > usize::MAX as u64 {
            return Err(LintConfigError::Parse(format!(
                "format {key} must be a number"
            )));
        }
        return Ok(value);
    }
    let text = raw
        .as_str()
        .ok_or_else(|| LintConfigError::Parse(format!("format {key} must be a number")))?;
    text.parse()
        .map_err(|_| LintConfigError::Parse(format!("format {key} must be a number")))
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
    fn json_format_typed_keys_parse_and_validate() {
        let text = r#"{"format":{"line_length":120,"tab_width":4,"style":"space","quote_style":"double","bracket_spacing":"tight"}}"#;
        let full = FlsConfig::parse_json(text).expect("parses");
        assert_eq!(full.format.line_length, Some(120));
        assert_eq!(full.format.tab_width, Some(4));
        assert_eq!(full.format.style, Some("space".to_string()));
        assert_eq!(full.format.quote_style, Some("double".to_string()));
        assert_eq!(full.format.bracket_spacing, Some("tight".to_string()));
    }

    #[test]
    fn json_format_unknown_keys_fail_fast() {
        // Intended break: raw future-rule store now rejects unknown keys.
        let err = FlsConfig::parse_json(r#"{"format":{"rules":{"future/rule":"terse"}}}"#)
            .expect_err("unknown format rule fails");
        assert_eq!(err, LintConfigError::UnknownRule("future/rule".to_string()));
        let err = FlsConfig::parse_json(r#"{"format":{"nope":1}}"#)
            .expect_err("unknown top-level format key fails");
        assert!(matches!(err, LintConfigError::Parse(_)));
    }

    #[test]
    fn json_format_ranges_validate() {
        let err = FlsConfig::parse_json(r#"{"format":{"line_length":0}}"#)
            .expect_err("zero length fails");
        assert!(matches!(err, LintConfigError::Parse(_)));
        let err =
            FlsConfig::parse_json(r#"{"format":{"tab_width":9}}"#).expect_err("wide tab fails");
        assert!(matches!(err, LintConfigError::Parse(_)));
        let err =
            FlsConfig::parse_json(r#"{"format":{"style":"two"}}"#).expect_err("bad style fails");
        assert!(matches!(err, LintConfigError::Parse(_)));
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
