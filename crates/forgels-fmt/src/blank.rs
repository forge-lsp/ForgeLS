//! Blank-line pass: top-level and function gaps plus empty blocks.
//!
//! Textual only, no AST import. Newline style is picked once per
//! file: CRLF when the text holds `\r\n`, else LF. Counts mirror the
//! lint rules: 2 blanks between top-level decls, 1 blank between
//! functions, single-line function pairs stay untouched. Other member
//! gaps are unchecked. Comments are never moved: edits touch blank
//! lines only, leading and trailing sides of comment blocks each get
//! the desired count.

use forgels_syntax::Span;

use super::edit::FormatEdit;
use super::whitespace::{is_blank_gap, split_lines};

// One split line with depth and comment flags.
struct LineInfo {
    start: usize,
    content_end: usize,
    end: usize,
    depth_before: usize,
    blank: bool,
    comment: bool,
}

fn line_infos(text: &str) -> Vec<LineInfo> {
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
        if in_string.is_some() {
            // Strings never span lines in practice; reset per line.
        }
    }
    let mut out = Vec::with_capacity(lines.len());
    for (pos, line) in lines.iter().enumerate() {
        let body = text.get(line.start..line.content_end).unwrap_or("");
        let blank = is_blank_gap(body);
        let stripped = body.trim_start_matches([' ', '\t']);
        let mut comment = false;
        if !blank
            && (stripped.starts_with("//")
                || stripped.starts_with("/*")
                || stripped.starts_with('*')
                || in_block_at[pos])
        {
            comment = true;
        }
        out.push(LineInfo {
            start: line.start,
            content_end: line.content_end,
            end: line.end,
            depth_before: depths[pos],
            blank,
            comment,
        });
    }
    out
}

fn is_function_start(body: &str) -> bool {
    let stripped = body.trim_start_matches([' ', '\t']);
    stripped.starts_with("function")
        && stripped
            .as_bytes()
            .get(8)
            .map(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'$')
            .unwrap_or(true)
}

fn body_has_both_braces(body: &str) -> bool {
    body.contains('{') && body.contains('}')
}

// Search back for a function header within a small window. Textual
// heuristic only: good for plain formatted sources.
fn prev_is_function(infos: &[LineInfo], text: &str, prev: usize) -> (bool, bool) {
    let Some(info) = infos.get(prev) else {
        return (false, false);
    };
    let body = text.get(info.start..info.content_end).unwrap_or("");
    if body.contains("function") && body_has_both_braces(body) {
        return (true, true);
    }
    let stripped = body.trim_start_matches([' ', '\t']);
    if !stripped.starts_with('}') {
        return (false, false);
    }
    let low = prev.saturating_sub(50);
    for cand in (low..prev).rev() {
        let Some(line) = infos.get(cand) else {
            continue;
        };
        if line.blank || line.comment {
            continue;
        }
        let cand_body = text.get(line.start..line.content_end).unwrap_or("");
        if is_function_start(cand_body) {
            return (true, false);
        }
    }
    (false, false)
}

fn next_is_function(infos: &[LineInfo], text: &str, next: usize) -> (bool, bool) {
    let Some(info) = infos.get(next) else {
        return (false, false);
    };
    let body = text.get(info.start..info.content_end).unwrap_or("");
    if !is_function_start(body) {
        return (false, false);
    }
    if body_has_both_braces(body) {
        return (true, true);
    }
    (true, false)
}

fn interior_has_comment(text: &str, start: usize, end: usize) -> bool {
    if start >= end || end > text.len() {
        return false;
    }
    let slice = text.get(start..end).unwrap_or("");
    let bytes = slice.as_bytes();
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
        if byte == b'"' || byte == b'\'' {
            in_string = Some(byte);
            idx += 1;
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'/') {
            return true;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            return true;
        }
        idx += 1;
    }
    false
}

// Collect blank-line edits. See module docs for rules.
pub fn blank_edits(text: &str) -> Vec<FormatEdit> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.trim().is_empty() {
        return Vec::new();
    }
    let newline: &str = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let infos = line_infos(text);
    if infos.len() < 2 {
        return Vec::new();
    }
    let mut edits = Vec::new();
    let code: Vec<usize> = infos
        .iter()
        .enumerate()
        .filter(|(_, info)| !info.blank && !info.comment)
        .map(|(pos, _)| pos)
        .collect();
    for pair in code.windows(2) {
        let prev = pair[0];
        let next = pair[1];
        if next <= prev {
            continue;
        }
        let gap_depth = if prev + 1 < next {
            infos[prev + 1].depth_before
        } else {
            infos[next].depth_before
        };
        let desired = if gap_depth == 0 {
            Some(2usize)
        } else if gap_depth == 1 {
            let (prev_func, prev_single) = prev_is_function(&infos, text, prev);
            if !prev_func {
                None
            } else {
                let (next_func, next_single) = next_is_function(&infos, text, next);
                if !next_func || (prev_single && next_single) {
                    None
                } else {
                    Some(1usize)
                }
            }
        } else {
            None
        };
        let Some(want) = desired else {
            continue;
        };
        let prev_end = infos[prev].end;
        let next_start = infos[next].start;
        if prev_end > next_start || next_start > text.len() {
            continue;
        }
        // Gap holds only blanks: replace whole span.
        let mut has_comment = false;
        for pos in (prev + 1)..next {
            if let Some(info) = infos.get(pos)
                && !info.blank
            {
                has_comment = true;
                break;
            }
        }
        if !has_comment {
            let current = if next == prev + 1 { 0 } else { next - prev - 1 };
            if current == want {
                continue;
            }
            let mut replace = String::new();
            for _ in 0..want {
                replace.push_str(newline);
            }
            edits.push(FormatEdit::new(
                Span::new(prev_end as u32, next_start as u32),
                replace,
            ));
            continue;
        }
        // Comment block inside: fix lines around it, never move it.
        let mut first_comment = next;
        let mut last_comment = prev;
        for pos in (prev + 1)..next {
            if let Some(info) = infos.get(pos)
                && !info.blank
            {
                if pos < first_comment {
                    first_comment = pos;
                }
                if pos > last_comment {
                    last_comment = pos;
                }
            }
        }
        if last_comment <= prev || first_comment >= next {
            continue;
        }
        let first_start = infos[first_comment].start;
        let last_end = infos[last_comment].end;
        if prev_end <= first_start {
            let leading_blanks = first_comment - prev - 1;
            if leading_blanks != want {
                let mut replace = String::new();
                for _ in 0..want {
                    replace.push_str(newline);
                }
                edits.push(FormatEdit::new(
                    Span::new(prev_end as u32, first_start as u32),
                    replace,
                ));
            }
        }
        if last_end <= next_start {
            let trailing_blanks = next - last_comment - 1;
            if trailing_blanks != want {
                let mut replace = String::new();
                for _ in 0..want {
                    replace.push_str(newline);
                }
                edits.push(FormatEdit::new(
                    Span::new(last_end as u32, next_start as u32),
                    replace,
                ));
            }
        }
    }
    edits.extend(empty_block_edits(text, &infos));
    edits
}

// Collapse multiline empty `{...}` to `{}` unless a comment sits inside.
fn empty_block_edits(text: &str, infos: &[LineInfo]) -> Vec<FormatEdit> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut idx = 0usize;
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
        if byte != b'{' {
            idx += 1;
            continue;
        }
        let open = idx;
        let Some(close) = match_empty_close(text, open) else {
            idx += 1;
            continue;
        };
        let inner = text.get(open + 1..close).unwrap_or("");
        if !inner.contains('\n') {
            idx = close + 1;
            continue;
        }
        if !inner.trim_matches([' ', '\t', '\n', '\r']).is_empty() {
            idx = close + 1;
            continue;
        }
        if interior_has_comment(text, open + 1, close) {
            idx = close + 1;
            continue;
        }
        // Skip when a comment line sits on the gap lines.
        let mut has_comment_line = false;
        for info in infos {
            if info.start >= close {
                break;
            }
            if info.end <= open + 1 {
                continue;
            }
            if info.start > open && info.end <= close + 1 && info.comment {
                has_comment_line = true;
                break;
            }
        }
        if has_comment_line {
            idx = close + 1;
            continue;
        }
        out.push(FormatEdit::new(
            Span::new((open + 1) as u32, close as u32),
            String::new(),
        ));
        idx = close + 1;
    }
    out
}

// Match `{` at `open` to its close when the inside is only whitespace.
fn match_empty_close(text: &str, open: usize) -> Option<usize> {
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
            return None;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            return None;
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
        } else if byte != b' ' && byte != b'\t' && byte != b'\n' && byte != b'\r' {
            return None;
        }
        idx += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_one(text: &str) -> String {
        let edits = blank_edits(text);
        crate::edit::apply(text, &edits).text
    }

    fn assert_blank_idempotent(text: &str) {
        let once = blank_one(text);
        let twice = blank_one(&once);
        assert_eq!(once, twice, "blank idempotent for {text:?}");
    }

    #[test]
    fn top_level_zero_grows_to_two() {
        let out = blank_one("contract A {}\ncontract B {}\n");
        assert_eq!(out, "contract A {}\n\n\ncontract B {}\n");
        assert_blank_idempotent("contract A {}\ncontract B {}\n");
    }

    #[test]
    fn top_level_three_shrinks_to_two() {
        let out = blank_one("contract A {}\n\n\n\ncontract B {}\n");
        assert_eq!(out, "contract A {}\n\n\ncontract B {}\n");
        assert_blank_idempotent("contract A {}\n\n\n\ncontract B {}\n");
    }

    #[test]
    fn top_level_two_stays() {
        let text = "contract A {}\n\n\ncontract B {}\n";
        assert_eq!(blank_one(text), text);
    }

    #[test]
    fn function_pair_zero_grows_to_one() {
        let text = "contract C {\nfunction f() {\nuint a;\n}\nfunction g() {\nuint b;\n}\n}\n";
        let out = blank_one(text);
        assert!(out.contains("}\n\nfunction g()"), "out: {out}");
        assert_blank_idempotent(text);
    }

    #[test]
    fn single_line_function_pair_untouched() {
        let text = "contract C {\nfunction f() {}\nfunction g() {}\n}\n";
        assert_eq!(blank_one(text), text);
    }

    #[test]
    fn comment_block_gets_blanks_around_it() {
        let text = "contract A {}\n// note\ncontract B {}\n";
        let out = blank_one(text);
        assert_eq!(out, "contract A {}\n\n\n// note\n\n\ncontract B {}\n");
        assert_blank_idempotent(text);
    }

    #[test]
    fn block_comment_anchoring_keeps_comment() {
        let text = "contract A {}\n/* note\nstill note */\ncontract B {}\n";
        let out = blank_one(text);
        assert!(out.contains("/* note"), "out: {out}");
        assert!(out.contains("contract A {}\n\n\n/*"), "out: {out}");
        assert!(out.contains("*/\n\n\ncontract B {}"), "out: {out}");
        assert_blank_idempotent(text);
    }

    #[test]
    fn empty_block_collapses() {
        let out = blank_one("contract A {\n\n}\n");
        assert_eq!(out, "contract A {}\n");
        assert_blank_idempotent("contract A {\n\n}\n");
    }

    #[test]
    fn empty_block_with_comment_stays() {
        let text = "contract A {\n// keep\n}\n";
        assert_eq!(blank_one(text), text);
    }

    #[test]
    fn crlf_gaps_use_crlf() {
        let out = blank_one("contract A {}\r\ncontract B {}\r\n");
        assert_eq!(out, "contract A {}\r\n\r\n\r\ncontract B {}\r\n");
        assert_blank_idempotent("contract A {}\r\ncontract B {}\r\n");
    }

    #[test]
    fn other_member_gaps_untouched() {
        let text = "contract C {\nuint a;\nuint b;\n}\n";
        assert_eq!(blank_one(text), text);
    }
}
