//! Brace-depth reindent: leading whitespace only.
//!
//! Tracks `{` and `}` outside strings and comments. Lines starting
//! with `}` dedent one level. Desired indent is options unit repeated
//! by depth. Lines inside wrap regions stay untouched except their
//! first lines so wrap output wins. Comment-only lines stay as-is;
//! code with a trailing comment still reindents its leading gap.
//! Assembly blocks stay verbatim from the `assembly` keyword through
//! its matching brace because opcode layout is custom. CRLF is kept:
//! only leading spaces and tabs are replaced. EOF newlines stay with
//! trim.

use forgels_syntax::Span;

use super::edit::FormatEdit;
use super::options::FormatOptions;
use super::whitespace::{is_blank_gap, split_lines};
use super::wrap::WrapRegions;

fn is_comment_line(body: &str, in_block: bool) -> bool {
    if in_block {
        return true;
    }
    let stripped = body.trim_start_matches([' ', '\t']);
    stripped.starts_with("//") || stripped.starts_with("/*") || stripped.starts_with('*')
}

fn contains_assembly_word(body: &str) -> bool {
    let bytes = body.as_bytes();
    let needle = b"assembly";
    if bytes.len() < needle.len() {
        return false;
    }
    for start in 0..=(bytes.len() - needle.len()) {
        if &bytes[start..start + needle.len()] != needle {
            continue;
        }
        let before = if start == 0 {
            None
        } else {
            Some(bytes[start - 1])
        };
        let after = bytes.get(start + needle.len()).copied();
        let before_ok = before
            .map(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'))
            .unwrap_or(true);
        let after_ok = after
            .map(|byte| !(byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'))
            .unwrap_or(true);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

// Collect reindent edits. See module docs for skip rules.
pub fn indent_edits(text: &str, regions: &WrapRegions, options: &FormatOptions) -> Vec<FormatEdit> {
    if text.is_empty() {
        return Vec::new();
    }
    let lines = split_lines(text);
    if lines.is_empty() {
        return Vec::new();
    }
    let mut depths = Vec::with_capacity(lines.len());
    let mut in_block_at = Vec::with_capacity(lines.len());
    let mut depth = 0usize;
    let mut in_block = false;
    for line in &lines {
        depths.push(depth);
        in_block_at.push(in_block);
        let body = text.get(line.start..line.content_end).unwrap_or("");
        let bytes = body.as_bytes();
        let mut idx = 0usize;
        let mut in_string: Option<u8> = None;
        let mut escaped = false;
        while idx < bytes.len() {
            let byte = bytes[idx];
            if let Some(quote) = in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == quote {
                    in_string = None;
                }
                idx += 1;
                continue;
            }
            if in_block {
                if byte == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                    in_block = false;
                    idx += 2;
                } else {
                    idx += 1;
                }
                continue;
            }
            if byte == b'"' || byte == b'\'' {
                in_string = Some(byte);
                idx += 1;
                continue;
            }
            if byte == b'/' && bytes.get(idx + 1) == Some(&b'/') {
                break;
            }
            if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
                in_block = true;
                idx += 2;
                continue;
            }
            if byte == b'{' {
                depth += 1;
            } else if byte == b'}' {
                depth = depth.saturating_sub(1);
            }
            idx += 1;
        }
    }
    let assembly_ranges = assembly_line_ranges(text, &lines);
    let unit = options.indent_unit();
    let mut edits = Vec::new();
    for (pos, line) in lines.iter().enumerate() {
        if assembly_ranges
            .iter()
            .any(|(lo, hi)| pos >= *lo && pos <= *hi)
        {
            continue;
        }
        if regions.skips_line(line.start) {
            continue;
        }
        let body = text.get(line.start..line.content_end).unwrap_or("");
        if is_blank_gap(body) {
            continue;
        }
        if is_comment_line(body, in_block_at[pos]) {
            continue;
        }
        let stripped = body.trim_start_matches([' ', '\t']);
        if stripped.is_empty() {
            continue;
        }
        let mut want_depth = depths[pos];
        if stripped.starts_with('}') && want_depth > 0 {
            want_depth -= 1;
        }
        let mut want = String::new();
        for _ in 0..want_depth {
            want.push_str(&unit);
        }
        let actual_len = body.len() - stripped.len();
        let actual = body.get(..actual_len).unwrap_or("");
        if actual == want {
            continue;
        }
        edits.push(FormatEdit::new(
            Span::new(line.start as u32, (line.start + actual_len) as u32),
            want,
        ));
    }
    edits
}

// Line index ranges for assembly blocks, inclusive. Each range runs
// from the line holding `assembly` through its matching brace.
fn assembly_line_ranges(text: &str, lines: &[super::whitespace::LineSpan]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pos, line) in lines.iter().enumerate() {
        let body = text.get(line.start..line.content_end).unwrap_or("");
        if !contains_assembly_word(body) {
            continue;
        }
        let keyword_off = body.find("assembly").unwrap_or(0);
        let search_from = line.start + keyword_off + "assembly".len();
        let Some(open) = next_brace_open(text, search_from) else {
            continue;
        };
        let Some(close) = match_brace_close(text, open) else {
            continue;
        };
        let mut end_pos = pos;
        for (cand, cand_line) in lines.iter().enumerate() {
            if cand < pos {
                continue;
            }
            if cand_line.start <= close && close < cand_line.end {
                end_pos = cand;
                break;
            }
            if cand_line.start > close {
                end_pos = cand.saturating_sub(1);
                break;
            }
        }
        out.push((pos, end_pos));
    }
    out
}

fn next_brace_open(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if from > bytes.len() {
        return None;
    }
    let mut idx = from;
    let mut in_string: Option<u8> = None;
    let mut escaped = false;
    let mut in_block = false;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                in_string = None;
            }
            idx += 1;
            continue;
        }
        if in_block {
            if byte == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                in_block = false;
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            in_string = Some(byte);
            idx += 1;
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'/') {
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            in_block = true;
            idx += 2;
            continue;
        }
        if byte == b'{' {
            return Some(idx);
        }
        if byte == b'}' {
            return None;
        }
        idx += 1;
    }
    None
}

fn match_brace_close(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'{') {
        return None;
    }
    let mut depth = 0u32;
    let mut idx = open;
    let mut in_string: Option<u8> = None;
    let mut escaped = false;
    let mut in_block = false;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if let Some(quote) = in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                in_string = None;
            }
            idx += 1;
            continue;
        }
        if in_block {
            if byte == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                in_block = false;
                idx += 2;
            } else {
                idx += 1;
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            in_string = Some(byte);
            idx += 1;
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'/') {
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            in_block = true;
            idx += 2;
            continue;
        }
        if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
        idx += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::IndentStyle;

    fn indent_one(text: &str, options: &FormatOptions) -> String {
        let regions = WrapRegions::default();
        let edits = indent_edits(text, &regions, options);
        crate::edit::apply(text, &edits).text
    }

    fn space_opts() -> FormatOptions {
        FormatOptions::default()
    }

    fn tab_opts() -> FormatOptions {
        FormatOptions {
            style: IndentStyle::Tab,
            ..Default::default()
        }
    }

    fn assert_indent_idempotent(text: &str, options: &FormatOptions) {
        let once = indent_one(text, options);
        let twice = indent_one(&once, options);
        assert_eq!(once, twice, "indent idempotent for {text:?}");
    }

    #[test]
    fn nesting_gets_spaces() {
        let out = indent_one("contract C {\nuint a;\n}\n", &space_opts());
        assert_eq!(out, "contract C {\n    uint a;\n}\n");
        assert_indent_idempotent("contract C {\nuint a;\n}\n", &space_opts());
    }

    #[test]
    fn closing_brace_dedents() {
        let out = indent_one("contract C {\n    uint a;\n    }\n", &space_opts());
        assert_eq!(out, "contract C {\n    uint a;\n}\n");
    }

    #[test]
    fn tab_style_uses_tabs() {
        let out = indent_one("contract C {\nuint a;\n}\n", &tab_opts());
        assert_eq!(out, "contract C {\n\tuint a;\n}\n");
        assert_indent_idempotent("contract C {\nuint a;\n}\n", &tab_opts());
    }

    #[test]
    fn inner_wrap_lines_are_skipped() {
        let text = "contract C { function f() public { greet(a, b,\n c); } }\n";
        let guard = crate::format::comment_spans(text);
        let (_, regions) = crate::wrap::wrap_edits(text, &guard, &space_opts());
        let out = crate::edit::apply(text, &indent_edits(text, &regions, &space_opts())).text;
        let second = out.lines().nth(1).unwrap_or("");
        assert_eq!(second, " c); } }");
    }

    #[test]
    fn comment_lines_stay_verbatim() {
        let text = "contract C {\n// note\n    uint a;\n/* block\n   inner */\n}\n";
        let out = indent_one(text, &space_opts());
        assert!(out.contains("// note\n"), "out: {out}");
        assert!(out.contains("/* block\n"), "out: {out}");
    }

    #[test]
    fn trailing_comment_still_reindents() {
        let out = indent_one("contract C {\nuint a; // hi\n}\n", &space_opts());
        assert_eq!(out, "contract C {\n    uint a; // hi\n}\n");
    }

    #[test]
    fn assembly_stays_verbatim() {
        let text = "contract C {\nfunction f() public {\nassembly {\nlet x := 1\n}\n}\n}\n";
        let out = indent_one(text, &space_opts());
        assert!(out.contains("assembly {\nlet x := 1\n}"), "out: {out}");
    }

    #[test]
    fn crlf_newlines_survive() {
        let out = indent_one("contract C {\r\nuint a;\r\n}\r\n", &space_opts());
        assert_eq!(out, "contract C {\r\n    uint a;\r\n}\r\n");
    }
}
