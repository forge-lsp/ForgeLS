use super::Severity;
use super::diagnostic::LintDiagnostic;

// One file worth of lint output. `text` is the source the diagnostic
// spans point into, `fixed` marks per-diagnostic write-mode results
// (parallel to `diagnostics`, missing entries read as false).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReport {
    pub path: String,
    pub text: String,
    pub diagnostics: Vec<LintDiagnostic>,
    pub fixed: Vec<bool>,
}

impl FileReport {
    pub fn new(path: String, text: String, diagnostics: Vec<LintDiagnostic>) -> Self {
        Self {
            path,
            text,
            diagnostics,
            fixed: Vec::new(),
        }
    }

    pub fn with_fixed(
        path: String,
        text: String,
        diagnostics: Vec<LintDiagnostic>,
        fixed: Vec<bool>,
    ) -> Self {
        Self {
            path,
            text,
            diagnostics,
            fixed,
        }
    }

    pub fn is_fixed(&self, index: usize) -> bool {
        self.fixed.get(index).copied().unwrap_or(false)
    }
}

fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

// Byte offset to 0-based line and character. Character units are UTF-16
// code units, matching the protocol default. Out-of-range offsets clamp
// to the nearest char boundary at or before the text end.
fn line_col(text: &str, offset: u32) -> (u32, u32) {
    let mut clamped = (offset as usize).min(text.len());
    while !text.is_char_boundary(clamped) {
        clamped -= 1;
    }
    let head = &text[..clamped];
    let line = head.bytes().filter(|&byte| byte == b'\n').count() as u32;
    let line_start = head.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let character = head[line_start..].encode_utf16().count() as u32;
    (line, character)
}

fn escape_json(raw: &str, out: &mut String) {
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

fn quoted(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    escape_json(raw, &mut out);
    out.push('"');
    out
}

fn point(text: &str, offset: u32) -> String {
    let (line, character) = line_col(text, offset);
    format!("{{\"line\":{line},\"character\":{character}}}")
}

fn span_range(text: &str, span: forgels_syntax::Span) -> String {
    format!(
        "{{\"start\":{},\"end\":{}}}",
        point(text, span.start),
        point(text, span.end)
    )
}

// Human report: one `path:line:col: severity[rule]: message` line per
// diagnostic, lines and columns 1-based.
pub fn report(files: &[FileReport]) -> String {
    let mut out = String::new();
    for file in files {
        for diag in &file.diagnostics {
            let (line, col) = line_col(&file.text, diag.span.start);
            out.push_str(&file.path);
            out.push(':');
            out.push_str(&(line + 1).to_string());
            out.push(':');
            out.push_str(&(col + 1).to_string());
            out.push_str(": ");
            out.push_str(severity_str(diag.severity));
            out.push('[');
            out.push_str(diag.rule.as_str());
            out.push_str("]: ");
            out.push_str(&diag.message);
            out.push('\n');
        }
    }
    out
}

// Machine report: per-file diagnostics with rule, severity, 0-based
// range, optional fix, and fixed flags, plus a summary with file,
// error, warning, and fixed counts. Files without diagnostics stay
// silent: only warn/err hits appear, so summary files counts the
// listed files only.
pub fn report_json(files: &[FileReport]) -> String {
    let listed: Vec<&FileReport> = files
        .iter()
        .filter(|file| !file.diagnostics.is_empty())
        .collect();
    let mut errors = 0u32;
    let mut warnings = 0u32;
    let mut fixed_total = 0u32;
    let mut out = String::from("{\"files\":[");
    for (file_index, file) in listed.iter().enumerate() {
        if file_index > 0 {
            out.push(',');
        }
        out.push_str("{\"path\":");
        out.push_str(&quoted(&file.path));
        out.push_str(",\"diagnostics\":[");
        for (diag_index, diag) in file.diagnostics.iter().enumerate() {
            if diag_index > 0 {
                out.push(',');
            }
            match diag.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
            }
            let fixed = file.is_fixed(diag_index);
            if fixed {
                fixed_total += 1;
            }
            out.push_str("{\"rule\":");
            out.push_str(&quoted(diag.rule.as_str()));
            out.push_str(",\"severity\":\"");
            out.push_str(severity_str(diag.severity));
            out.push_str("\",\"message\":");
            out.push_str(&quoted(&diag.message));
            out.push_str(",\"range\":");
            out.push_str(&span_range(&file.text, diag.span));
            if let Some(fix) = &diag.fix {
                out.push_str(",\"fix\":{\"applicability\":\"");
                out.push_str(fix.applicability.as_str());
                out.push_str("\",\"edits\":[{\"range\":");
                out.push_str(&span_range(&file.text, fix.span));
                out.push_str(",\"newText\":");
                out.push_str(&quoted(&fix.replacement));
                out.push_str("}]}");
            }
            out.push_str(",\"fixed\":");
            out.push_str(if fixed { "true" } else { "false" });
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("],\"summary\":{\"files\":");
    out.push_str(&listed.len().to_string());
    out.push_str(",\"errors\":");
    out.push_str(&errors.to_string());
    out.push_str(",\"warnings\":");
    out.push_str(&warnings.to_string());
    out.push_str(",\"fixed\":");
    out.push_str(&fixed_total.to_string());
    out.push_str("}}");
    out
}

#[cfg(test)]
mod tests {
    use forgels_syntax::Span;

    use super::super::{LintFix, RuleId};
    use super::*;

    fn file() -> FileReport {
        let text = "contract A {}\nuint x;\n".to_string();
        let at = text.find('x').expect("x offset") as u32;
        FileReport::new(
            "a.sol".to_string(),
            text,
            vec![
                LintDiagnostic::warning(
                    Span::new(at, at + 1),
                    "unused `x`".to_string(),
                    RuleId::UNUSED,
                )
                .with_fix(LintFix::safe(Span::new(at, at + 1), String::new())),
                LintDiagnostic::error(
                    Span::new(0, 8),
                    "bad \"name\"".to_string(),
                    RuleId::PARSE_ERROR,
                ),
            ],
        )
    }

    #[test]
    fn human_report_is_one_line_per_diagnostic() {
        assert_eq!(
            report(&[file()]),
            "a.sol:2:6: warning[correctness/unused]: unused `x`\n\
             a.sol:1:1: error[correctness/parse-error]: bad \"name\"\n"
        );
    }

    #[test]
    fn json_report_has_ranges_fix_fixed_and_summary() {
        assert_eq!(
            report_json(&[file()]),
            "{\"files\":[{\"path\":\"a.sol\",\"diagnostics\":[\
             {\"rule\":\"correctness/unused\",\"severity\":\"warning\",\"message\":\"unused `x`\",\
             \"range\":{\"start\":{\"line\":1,\"character\":5},\"end\":{\"line\":1,\"character\":6}},\
             \"fix\":{\"applicability\":\"safe\",\"edits\":[{\"range\":\
             {\"start\":{\"line\":1,\"character\":5},\"end\":{\"line\":1,\"character\":6}},\
             \"newText\":\"\"}]},\"fixed\":false},\
             {\"rule\":\"correctness/parse-error\",\"severity\":\"error\",\"message\":\"bad \\\"name\\\"\",\
             \"range\":{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":0,\"character\":8}},\
             \"fixed\":false}]}],\"summary\":{\"files\":1,\"errors\":1,\"warnings\":1,\"fixed\":0}}"
        );
    }

    #[test]
    fn json_report_marks_fixed_in_write_mode() {
        let mut file = file();
        file.fixed = vec![true, false];
        let json = report_json(&[file]);
        assert!(json.contains("\"fixed\":true"));
        assert!(json.contains("\"fixed\":1"));
    }

    #[test]
    fn json_report_parses_and_counts_summary() {
        let value: serde_json::Value =
            serde_json::from_str(&report_json(&[file()])).expect("valid json");
        assert_eq!(
            value.pointer("/summary").expect("has summary"),
            &serde_json::json!({"files": 1, "errors": 1, "warnings": 1, "fixed": 0})
        );
        let diags = value
            .pointer("/files/0/diagnostics")
            .expect("has diagnostics")
            .as_array()
            .expect("array");
        assert_eq!(diags.len(), 2);
        assert_eq!(
            diags[0].pointer("/fix/applicability").expect("has fix"),
            &serde_json::json!("safe")
        );
        assert!(diags[1].get("fix").is_none());
    }

    #[test]
    fn empty_report_has_empty_summary() {
        assert_eq!(report(&[]), "");
        assert_eq!(
            report_json(&[]),
            "{\"files\":[],\"summary\":{\"files\":0,\"errors\":0,\"warnings\":0,\"fixed\":0}}"
        );
    }

    #[test]
    fn json_report_skips_clean_files() {
        let clean = FileReport::new(
            "clean.sol".to_string(),
            "contract A {}\n".to_string(),
            vec![],
        );
        let value: serde_json::Value =
            serde_json::from_str(&report_json(&[clean, file()])).expect("valid json");
        let files = value
            .pointer("/files")
            .expect("has files")
            .as_array()
            .expect("array");
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].pointer("/path").expect("has path"),
            &serde_json::json!("a.sol")
        );
        assert_eq!(
            value.pointer("/summary/files").expect("has count"),
            &serde_json::json!(1)
        );
    }
}
