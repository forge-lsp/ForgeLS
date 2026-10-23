//! Guest JSON shapes and host-side conversion.
//!
//! Requests are `{"path","text"}` plus format options. Lint answers
//! carry `diagnostics` with rule, message, span, severity, and an
//! optional fix. Format answers carry `edits`. Conversion clamps
//! spans, drops empty rule IDs, maps severity strings, and turns
//! fixes into `LintFix::safe` plus edits into `FormatEdit`.

use std::collections::HashMap;

use forgels_fmt::FormatEdit;
use forgels_lint::{LintDiagnostic, LintFix, RuleId, Severity};
use forgels_syntax::Span;

/// Build lint request JSON for one file.
pub fn lint_request_json(path: &str, text: &str) -> String {
    serde_json::json!({"path": path, "text": text}).to_string()
}

/// Build format request JSON for one file with options.
pub fn format_request_json(
    path: &str,
    text: &str,
    line_length: usize,
    tab_width: usize,
    style: &str,
) -> String {
    serde_json::json!({
        "path": path,
        "text": text,
        "line_length": line_length,
        "tab_width": tab_width,
        "style": style,
    })
    .to_string()
}

/// Map a severity string to `Severity`. Accepts error/err and
/// warning/warn case-insensitive; unknown falls back to Warning.
pub fn parse_severity(raw: &str) -> Severity {
    let lower = raw.to_ascii_lowercase();
    match lower.as_str() {
        "error" | "err" => Severity::Error,
        "warning" | "warn" => Severity::Warning,
        _ => Severity::Warning,
    }
}

/// Resolve a rule string to `RuleId`. Builtins win, then the guest
/// cache, else leak once for well formed IDs. Empty or whitespace
/// IDs return `None` so callers drop the diagnostic.
pub fn rule_id_for(raw: &str, cache: &mut HashMap<String, RuleId>) -> Option<RuleId> {
    if raw.is_empty() || raw.chars().any(|ch| ch.is_whitespace()) {
        return None;
    }
    if let Some(known) = RuleId::all().iter().find(|id| id.as_str() == raw) {
        return Some(*known);
    }
    if let Some(cached) = cache.get(raw) {
        return Some(*cached);
    }
    if !RuleId::is_guest_id(raw) {
        return None;
    }
    let leaked: &'static str = Box::leak(raw.to_string().into_boxed_str());
    let id = RuleId::from_leaked(leaked);
    cache.insert(raw.to_string(), id);
    Some(id)
}

fn u32_field(value: &serde_json::Value, key: &str) -> Option<u32> {
    value
        .get(key)?
        .as_u64()
        .and_then(|num| u32::try_from(num).ok())
}

/// Parse guest lint output into diagnostics. Spans clamp to `file_len`,
/// empty rule IDs drop, severity strings map, fixes become safe.
pub fn parse_lint_output(
    output: &str,
    file_len: u32,
    cache: &mut HashMap<String, RuleId>,
) -> Result<Vec<LintDiagnostic>, String> {
    let value: serde_json::Value =
        serde_json::from_str(output).map_err(|err| format!("fls: guest lint json: {err}"))?;
    let Some(list) = value.get("diagnostics").and_then(|item| item.as_array()) else {
        return Err(String::from("fls: guest lint json: missing diagnostics"));
    };
    let mut out = Vec::new();
    for entry in list {
        let Some(rule_raw) = entry.get("rule").and_then(|item| item.as_str()) else {
            continue;
        };
        let Some(rule) = rule_id_for(rule_raw, cache) else {
            continue;
        };
        let message = entry
            .get("message")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string();
        let start = u32_field(entry, "start").unwrap_or(0);
        let end = u32_field(entry, "end").unwrap_or(start);
        let span = Span::new(start, end).clamp(file_len);
        let severity = entry
            .get("severity")
            .and_then(|item| item.as_str())
            .map(parse_severity)
            .unwrap_or(Severity::Warning);
        let mut diag = LintDiagnostic::from_parts(span, message, rule, severity);
        if let Some(fix) = entry.get("fix") {
            let fix_start = u32_field(fix, "start").unwrap_or(span.start);
            let fix_end = u32_field(fix, "end").unwrap_or(span.end);
            let replacement = fix
                .get("replacement")
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .to_string();
            let fix_span = Span::new(fix_start, fix_end).clamp(file_len);
            diag.fix = Some(LintFix::safe(fix_span, replacement));
        }
        out.push(diag);
    }
    Ok(out)
}

/// Parse guest format output into edits. Spans clamp to `file_len`.
pub fn parse_format_output(output: &str, file_len: u32) -> Result<Vec<FormatEdit>, String> {
    let value: serde_json::Value =
        serde_json::from_str(output).map_err(|err| format!("fls: guest format json: {err}"))?;
    if let Some(text) = value.get("text").and_then(|item| item.as_str()) {
        if text.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![FormatEdit::new(
            Span::new(0, file_len),
            text.to_string(),
        )]);
    }
    let Some(list) = value.get("edits").and_then(|item| item.as_array()) else {
        return Err(String::from("fls: guest format json: missing edits"));
    };
    let mut out = Vec::new();
    for entry in list {
        let start = u32_field(entry, "start").unwrap_or(0);
        let end = u32_field(entry, "end").unwrap_or(start);
        let replacement = entry
            .get("replacement")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .to_string();
        out.push(FormatEdit::new(
            Span::new(start, end).clamp(file_len),
            replacement,
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rule_id_drops_diagnostic() {
        let mut cache = HashMap::new();
        assert!(rule_id_for("", &mut cache).is_none());
        assert!(rule_id_for("   ", &mut cache).is_none());
        assert!(rule_id_for("has space/x", &mut cache).is_none());
    }

    #[test]
    fn guest_id_shape_accepts_slashed_names() {
        let mut cache = HashMap::new();
        assert!(rule_id_for("example/guest-rule", &mut cache).is_some());
        assert!(rule_id_for("no-slash", &mut cache).is_none());
    }

    #[test]
    fn span_clamps_to_file_len() {
        let mut cache = HashMap::new();
        let output = r#"{"diagnostics":[{"rule":"example/guest-rule","message":"hi","start":0,"end":9999,"severity":"warning"}]}"#;
        let diags = parse_lint_output(output, 10, &mut cache).expect("parses");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].span, Span::new(0, 10));
    }

    #[test]
    fn severity_strings_map_both_forms() {
        assert_eq!(parse_severity("error"), Severity::Error);
        assert_eq!(parse_severity("err"), Severity::Error);
        assert_eq!(parse_severity("warning"), Severity::Warning);
        assert_eq!(parse_severity("warn"), Severity::Warning);
        assert_eq!(parse_severity("other"), Severity::Warning);
    }

    #[test]
    fn fix_becomes_safe_with_clamped_span() {
        let mut cache = HashMap::new();
        let output = r#"{"diagnostics":[{"rule":"example/guest-rule","message":"hi","start":0,"end":2,"severity":"error","fix":{"start":0,"end":99,"replacement":"x"}}]}"#;
        let diags = parse_lint_output(output, 5, &mut cache).expect("parses");
        let fix = diags[0].fix.clone().expect("fix attached");
        assert_eq!(fix.span, Span::new(0, 5));
        assert_eq!(fix.replacement, "x");
    }
}
