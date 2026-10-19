use forgels_syntax::Span;

use super::edit::FormatEdit;
use super::whitespace::{is_blank_gap, split_lines, trailing_span};

fn overlaps(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

fn intersects_guard(span: Span, guard: &[Span]) -> bool {
    guard.iter().any(|other| overlaps(span, *other))
}

// Collect P0 edits: trim trailing spaces and tabs plus exactly one
// trailing newline. Empty stays empty; whitespace-only becomes one
// `\n`; CRLF bodies keep `\r\n`, only the EOF newline is LF. Guard
// spans (comments) keep their trailing spaces: covered edits are
// dropped or start after the guard.
pub fn trim_edits(text: &str, guard: &[Span]) -> Vec<FormatEdit> {
    if text.is_empty() {
        return Vec::new();
    }
    if text
        .bytes()
        .all(|byte| byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r')
    {
        if text == "\n" {
            return Vec::new();
        }
        return vec![FormatEdit::new(
            Span::new(0, text.len() as u32),
            "\n".to_string(),
        )];
    }
    let lines = split_lines(text);
    let Some(last_kept) = lines.iter().rfind(|line| {
        let body = text.get(line.start..line.content_end).unwrap_or("");
        !is_blank_gap(body)
    }) else {
        return vec![FormatEdit::new(
            Span::new(0, text.len() as u32),
            "\n".to_string(),
        )];
    };
    let mut edits = Vec::new();
    for line in &lines {
        if line.start >= last_kept.start {
            break;
        }
        let Some(span) = trailing_span(text, line.start, line.content_end) else {
            continue;
        };
        if intersects_guard(span, guard) {
            continue;
        }
        edits.push(FormatEdit::new(span, String::new()));
    }
    let body = text
        .get(last_kept.start..last_kept.content_end)
        .unwrap_or("");
    let trimmed_len = body.trim_end_matches([' ', '\t']).len();
    let mut edge = last_kept.start + trimmed_len;
    if let Some(span) = trailing_span(text, last_kept.start, last_kept.content_end)
        && intersects_guard(span, guard)
    {
        edge = last_kept.content_end;
    }
    let tail = text.get(edge..).unwrap_or("");
    if tail != "\n" {
        edits.push(FormatEdit::new(
            Span::new(edge as u32, text.len() as u32),
            "\n".to_string(),
        ));
    }
    edits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply_all(text: &str) -> String {
        let edits = trim_edits(text, &[]);
        super::super::edit::apply(text, &edits).text
    }

    #[test]
    fn trims_trailing_spaces() {
        assert_eq!(apply_all("a;   \n"), "a;\n");
    }

    #[test]
    fn trims_trailing_tabs() {
        assert_eq!(apply_all("a;\t\t\n"), "a;\n");
    }

    #[test]
    fn trims_blank_line_bodies() {
        assert_eq!(apply_all("a;\n   \n"), "a;\n");
    }

    #[test]
    fn adds_missing_eof_newline() {
        assert_eq!(apply_all("a;"), "a;\n");
    }

    #[test]
    fn collapses_extra_eof_newlines() {
        assert_eq!(apply_all("a;\n\n\n"), "a;\n");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(apply_all(""), "");
    }

    #[test]
    fn whitespace_only_becomes_single_newline() {
        assert_eq!(apply_all("   \n \t\n"), "\n");
        assert_eq!(apply_all("\n"), "\n");
    }

    #[test]
    fn crlf_body_preserved_with_lf_eof() {
        assert_eq!(apply_all("a;\r\nb;\r\n"), "a;\r\nb;\n");
    }

    #[test]
    fn guard_keeps_comment_trailing_spaces() {
        let text = "// hi   \n";
        let guard = vec![Span::new(0, 7)];
        let edits = trim_edits(text, &guard);
        assert!(edits.is_empty());
    }
}
