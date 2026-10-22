use forgels_syntax::Span;

use super::edit::FormatEdit;
use super::options::FormatOptions;

pub const MAX_LINE_LEN: usize = 120;
pub const CONTINUATION_INDENT: usize = 4;

// Byte spans of calls that wrap this pass. Indent skips inner lines
// of these spans and reindents only their first lines.
#[derive(Debug, Clone, Default)]
pub struct WrapRegions {
    pub spans: Vec<Span>,
}

impl WrapRegions {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    // True when a line starting at `line_start` sits inside a wrapped
    // call but does not hold its opening paren.
    pub fn skips_line(&self, line_start: usize) -> bool {
        if self.spans.is_empty() {
            return false;
        }
        self.spans.iter().any(|span| {
            let start = span.start as usize;
            let end = span.end as usize;
            start < line_start && line_start < end
        })
    }
}

#[derive(Debug, Clone)]
struct WrapTarget {
    open: usize,
    close: usize,
    args: Vec<(usize, usize)>,
}

// Collect canonical wrap edits plus wrapped regions. Textual scan
// only: no AST import keeps `forgels-fmt` dependency light. AST
// marker nodes store no per-arg spans today, so text slices give
// exact args without grammar churn. Contract heritage and modifier
// params wrap as plain calls: the scan classifies by name shape
// only. Width uses options line length with tab-aware counting;
// continuation uses options indent unit.
pub fn wrap_edits(
    text: &str,
    guard: &[Span],
    options: &FormatOptions,
) -> (Vec<FormatEdit>, WrapRegions) {
    if text.is_empty() {
        return (Vec::new(), WrapRegions::default());
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut targets = Vec::new();
    let mut index = 0usize;
    while index < text.len() {
        let Some(rel) = text[index..].find('(') else {
            break;
        };
        let open = index + rel;
        if let Some(target) = collect_target(text, open)
            && !targets
                .iter()
                .any(|kept: &WrapTarget| kept.open == target.open && kept.close == target.close)
        {
            targets.push(target);
        }
        index = open + 1;
    }
    targets.sort_by(|left, right| {
        left.open
            .cmp(&right.open)
            .then(right.close.cmp(&left.close))
    });
    let mut kept: Vec<WrapTarget> = Vec::new();
    for target in targets {
        if kept.iter().any(|prev| overlaps(prev, &target)) {
            continue;
        }
        kept.push(target);
    }
    let mut edits = Vec::new();
    let mut regions = WrapRegions::default();
    for target in &kept {
        let Some(edit) = build_edit(text, target, guard, newline, options) else {
            continue;
        };
        let close_end = target.close + 1;
        if target.open < close_end && close_end <= text.len() {
            regions.spans.push(Span::new(
                target.open as u32,
                close_end.min(u32::MAX as usize) as u32,
            ));
        }
        edits.push(edit);
    }
    (edits, regions)
}

fn overlaps(kept: &WrapTarget, next: &WrapTarget) -> bool {
    kept.open <= next.close && kept.close >= next.open
}

fn collect_target(text: &str, open: usize) -> Option<WrapTarget> {
    if !is_wrap_open(text, open) {
        return None;
    }
    let (close, commas) = scan_parens(text, open)?;
    if contains_brace(text, open + 1, close) {
        return None;
    }
    let args = split_args(text, open + 1, close, &commas);
    if args.is_empty() {
        return None;
    }
    // Single-arg already-multiline stays as-is; over-120 still wraps.
    // Multi-arg uses over-120 or already-split. Checked in build.
    Some(WrapTarget { open, close, args })
}

fn build_edit(
    text: &str,
    target: &WrapTarget,
    guard: &[Span],
    newline: &str,
    options: &FormatOptions,
) -> Option<FormatEdit> {
    let close_end = target.close + 1;
    if target.open >= close_end || close_end > text.len() {
        return None;
    }
    let list_span = Span::new(target.open as u32, close_end as u32);
    if guard.iter().any(|other| spans_overlap(list_span, *other)) {
        return None;
    }
    let slice = text.get(target.open..close_end)?;
    let multiline = slice.contains('\n');
    let line_len = line_len_at(text, target.open, options);
    let overlong = line_len > options.line_length;
    let single = target.args.len() == 1;
    let trigger = if single {
        overlong
    } else {
        overlong || multiline
    };
    if !trigger {
        return None;
    }
    let indent = line_indent(text, target.open);
    let cont = format!("{indent}{}", options.indent_unit());
    let mut arg_texts = Vec::new();
    for (start, end) in &target.args {
        let raw = text.get(*start..*end)?;
        let trimmed = raw.trim_matches([' ', '\t', '\n', '\r']);
        if trimmed.is_empty() {
            continue;
        }
        arg_texts.push(trimmed.to_string());
    }
    if arg_texts.is_empty() {
        return None;
    }
    if single && arg_texts.len() != 1 {
        return None;
    }
    // Already canonical stays untouched so double format is stable.
    if is_canonical(text, target, &indent, &cont, newline) {
        return None;
    }
    let mut out = String::from("(");
    for (idx, arg) in arg_texts.iter().enumerate() {
        out.push_str(newline);
        out.push_str(&cont);
        out.push_str(arg);
        if idx + 1 < arg_texts.len() {
            out.push(',');
        }
    }
    out.push_str(newline);
    out.push_str(&indent);
    out.push(')');
    Some(FormatEdit::new(list_span, out))
}

fn spans_overlap(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

fn line_len_at(text: &str, offset: usize, options: &FormatOptions) -> usize {
    let start = text[..offset.min(text.len())]
        .rfind('\n')
        .map(|pos| pos + 1)
        .unwrap_or(0);
    let end = text[offset.min(text.len())..]
        .find('\n')
        .map(|pos| offset + pos)
        .unwrap_or(text.len());
    let line = text.get(start..end).unwrap_or("");
    let trimmed = line.strip_suffix('\r').unwrap_or(line);
    options.line_width(trimmed)
}

fn line_indent(text: &str, offset: usize) -> String {
    let start = text[..offset.min(text.len())]
        .rfind('\n')
        .map(|pos| pos + 1)
        .unwrap_or(0);
    let full = text.get(start..).unwrap_or("");
    let mut indent = String::new();
    for ch in full.chars() {
        if ch == ' ' || ch == '\t' {
            indent.push(ch);
        } else {
            break;
        }
    }
    indent
}

fn is_canonical(text: &str, target: &WrapTarget, indent: &str, cont: &str, newline: &str) -> bool {
    let close_end = target.close + 1;
    let Some(current) = text.get(target.open..close_end) else {
        return false;
    };
    let mut arg_texts = Vec::new();
    for (start, end) in &target.args {
        let Some(raw) = text.get(*start..*end) else {
            return false;
        };
        let trimmed = raw.trim_matches([' ', '\t', '\n', '\r']);
        if trimmed.is_empty() {
            continue;
        }
        arg_texts.push(trimmed);
    }
    let mut expect = String::from("(");
    for (idx, arg) in arg_texts.iter().enumerate() {
        expect.push_str(newline);
        expect.push_str(cont);
        expect.push_str(arg);
        if idx + 1 < arg_texts.len() {
            expect.push(',');
        }
    }
    expect.push_str(newline);
    expect.push_str(indent);
    expect.push(')');
    current == expect
}

fn split_args(text: &str, start: usize, close: usize, commas: &[usize]) -> Vec<(usize, usize)> {
    if start >= close || close > text.len() {
        return Vec::new();
    }
    let inner = text.get(start..close).unwrap_or("");
    if inner.trim_matches([' ', '\t', '\n', '\r']).is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut prev = start;
    for comma in commas {
        if *comma < start || *comma >= close {
            continue;
        }
        out.push((prev, *comma));
        prev = comma + 1;
    }
    out.push((prev, close));
    out
}

fn contains_brace(text: &str, start: usize, close: usize) -> bool {
    let Some(slice) = text.get(start..close) else {
        return false;
    };
    let mut in_string: Option<u8> = None;
    let mut escaped = false;
    let bytes = slice.as_bytes();
    let mut idx = 0usize;
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
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            idx += 2;
            while idx < bytes.len() {
                if bytes[idx] == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                    idx += 2;
                    break;
                }
                idx += 1;
            }
            continue;
        }
        if byte == b'{' || byte == b'}' {
            return true;
        }
        idx += 1;
    }
    false
}

// Balanced scan from `(` to its match. Skips strings and comments,
// tracks nested parens plus brackets so commas inside stay hidden.
// Returns close offset plus top-level comma offsets.
fn scan_parens(text: &str, open: usize) -> Option<(usize, Vec<usize>)> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut paren = 0u32;
    let mut bracket = 0u32;
    let mut commas = Vec::new();
    let mut idx = open;
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
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(idx + 1) == Some(&b'*') {
            idx += 2;
            let mut closed = false;
            while idx < bytes.len() {
                if bytes[idx] == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                    idx += 2;
                    closed = true;
                    break;
                }
                idx += 1;
            }
            if !closed {
                return None;
            }
            continue;
        }
        match byte {
            b'(' => paren += 1,
            b')' => {
                if paren == 0 {
                    return None;
                }
                paren -= 1;
                if paren == 0 {
                    return Some((idx, commas));
                }
            }
            b'[' => {
                if paren >= 1 {
                    bracket += 1;
                }
            }
            b']' => {
                if paren >= 1 {
                    if bracket == 0 {
                        return None;
                    }
                    bracket -= 1;
                }
            }
            b',' if paren == 1 && bracket == 0 => {
                commas.push(idx);
            }
            b',' => {}
            _ => {}
        }
        idx += 1;
    }
    None
}

fn is_wrap_open(text: &str, open: usize) -> bool {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return false;
    }
    let Some(before) = skip_spaces_back(text, open) else {
        return false;
    };
    // Options form `name{...}(args)`: glue keeps `{...}` on the callee
    // line, wrapping starts at `(`.
    if text.as_bytes().get(before) == Some(&b'}') {
        return is_wrap_options(text, before);
    }
    let Some((word, _)) = word_before(text, before) else {
        return false;
    };
    if word == "returns" || word == "constructor" {
        return true;
    }
    if word == "mapping"
        || word == "type"
        || word == "if"
        || word == "for"
        || word == "while"
        || word == "catch"
        || word == "receive"
        || word == "fallback"
        || word == "assembly"
        || word == "using"
        || word == "modifier"
        || word == "return"
        || word == "revert"
        || word == "emit"
        || word == "new"
        || word == "event"
        || word == "error"
        || word == "function"
    {
        // Bare keyword directly before `(` is control, glue, or decl
        // without a name. None of those wrap here.
        return false;
    }
    // Named calls, member calls, emit/revert/new prefixes, contract
    // heritage and modifier params all wrap as plain calls.
    true
}

fn is_wrap_options(text: &str, brace_end: usize) -> bool {
    let Some(brace_start) = match_brace_back(text, brace_end) else {
        return false;
    };
    let Some(before_brace) = skip_spaces_back(text, brace_start) else {
        return false;
    };
    let Some(_) = word_before(text, before_brace) else {
        return false;
    };
    true
}

fn skip_spaces_back(text: &str, mut end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if end == 0 || end > text.len() {
        if end == 0 {
            return None;
        }
        end = text.len();
    }
    let mut idx = end;
    while idx > 0 {
        let byte = bytes[idx - 1];
        if byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r' {
            idx -= 1;
        } else {
            break;
        }
    }
    if idx == 0 { None } else { Some(idx - 1) }
}

fn word_before(text: &str, end: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if end >= text.len() {
        return None;
    }
    let byte = bytes[end];
    if !is_word_byte(byte) {
        return None;
    }
    let mut start = end;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    let word = text.get(start..end + 1)?.to_string();
    Some((word, start))
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn match_brace_back(text: &str, end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(end) != Some(&b'}') {
        return None;
    }
    let mut depth = 0u32;
    let mut idx = end + 1;
    while idx > 0 {
        idx -= 1;
        let byte = bytes[idx];
        if byte == b'}' {
            depth += 1;
        } else if byte == b'{' {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
        if idx == 0 {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::comment_spans;

    fn wrap_one(text: &str) -> String {
        let guard = comment_spans(text);
        let opts = FormatOptions::default();
        let (edits, _) = wrap_edits(text, &guard, &opts);
        crate::edit::apply(text, &edits).text
    }

    fn wrap_with(text: &str, opts: &FormatOptions) -> (String, WrapRegions) {
        let guard = comment_spans(text);
        let (edits, regions) = wrap_edits(text, &guard, opts);
        (crate::edit::apply(text, &edits).text, regions)
    }

    fn long_call() -> String {
        format!(
            "contract C {{ function f() public {{ {}(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo); }} }}",
            "greet"
        )
    }

    fn assert_idempotent(text: &str) {
        let once = wrap_one(text);
        let twice = wrap_one(&once);
        assert_eq!(once, twice, "wrap idempotent for {text:?}");
    }

    #[test]
    fn short_call_stays_single_line() {
        let text = "contract C { function f() public { greet(a, b); } }\n";
        assert_eq!(wrap_one(text), text);
        assert_idempotent(text);
    }

    #[test]
    fn empty_call_never_wraps() {
        let text = "contract C { function f() public { greet(); } }\n";
        assert_eq!(wrap_one(text), text);
    }

    #[test]
    fn long_plain_call_wraps_canonical() {
        let text = long_call();
        let out = wrap_one(&text);
        assert!(out.contains("greet(\n"), "out: {out}");
        assert!(out.contains("\n    aaaa,"), "out: {out}");
        assert!(out.contains("\n)"), "out: {out}");
        assert_idempotent(&text);
        assert_idempotent(&out);
    }

    #[test]
    fn long_member_call_wraps() {
        let text = "contract C { function f() public { obj.greet(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp, qqqq); } }\n";
        assert!(text.len() > MAX_LINE_LEN);
        let out = wrap_one(text);
        assert!(out.contains("obj.greet(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_emit_wraps() {
        let text = "contract C { function f() public { emit Transfer(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n";
        let out = wrap_one(text);
        assert!(out.contains("emit Transfer(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_revert_wraps() {
        let text = "contract C { function f() public { revert Insufficient(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm); } }\n";
        let out = wrap_one(text);
        assert!(out.contains("revert Insufficient(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_new_wraps() {
        let text = "contract C { function f() public { new Vault(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo); } }\n";
        let out = wrap_one(text);
        assert!(out.contains("new Vault(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_function_params_wrap() {
        let text = "contract C { function add(uint256 aaaa, uint256 bbbb, uint256 cccc, uint256 dddd, uint256 eeee, uint256 ffff, uint256 gggg) public {} }\n";
        let out = wrap_one(text);
        assert!(out.contains("function add(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_event_params_wrap() {
        let text = "contract C { event Transfer(address aaaa, address bbbb, address cccc, address dddd, address eeee, address ffff, address gggg, address hhhh); }\n";
        let out = wrap_one(text);
        assert!(out.contains("event Transfer(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_error_params_wrap() {
        let text = "contract C { error Insufficient(uint256 aaaa, uint256 bbbb, uint256 cccc, uint256 dddd, uint256 eeee, uint256 ffff, uint256 gggg); }\n";
        let out = wrap_one(text);
        assert!(out.contains("error Insufficient(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn long_returns_wraps_like_calls() {
        let text = "contract C { function f() public returns (uint256 aaaa, uint256 bbbb, uint256 cccc, uint256 dddd, uint256 eeee, uint256 ffff, uint256 gggg) {} }\n";
        let out = wrap_one(text);
        assert!(
            out.contains("returns (\n") || out.contains("returns(\n"),
            "out: {out}"
        );
        assert_idempotent(text);
    }

    #[test]
    fn single_arg_short_stays_even_when_split() {
        let text = "contract C { function f() public { greet(\n    a\n); } }\n";
        assert_eq!(wrap_one(text), text);
    }

    #[test]
    fn single_arg_long_wraps() {
        let base = "a".repeat(130);
        let text = format!("contract C {{ function f() public {{ greet({base}); }} }}\n");
        let out = wrap_one(&text);
        assert!(out.contains("greet(\n"), "out: {out}");
        assert_idempotent(&text);
    }

    #[test]
    fn already_multiline_normalizes_to_canonical() {
        let text = "contract C { function f() public { greet(a, b,\n c); } }\n";
        // Short line but already split: multi-arg normalizes.
        let out = wrap_one(text);
        assert!(out.contains("greet(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn comment_inside_blocks_wrap() {
        let text = "contract C { function f() public { greet(aaaa, /* note */ bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn); } }\n";
        assert_eq!(wrap_one(text), text);
    }

    #[test]
    fn options_stay_glued_and_args_wrap() {
        let text = "contract C { function f() public { greet{value: 1}(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n";
        let out = wrap_one(text);
        assert!(out.contains("greet{value: 1}(\n"), "out: {out}");
        assert_idempotent(text);
    }

    #[test]
    fn control_parens_never_wrap() {
        let text = "contract C { function f() public { if (true) {} for (uint i = 0; i < 10; i++) {} } }\n";
        assert_eq!(wrap_one(text), text);
    }

    #[test]
    fn mapping_paren_never_wraps() {
        let text = "contract C { mapping(address => uint) balances; }\n";
        assert_eq!(wrap_one(text), text);
    }

    #[test]
    fn outer_wins_over_inner() {
        let text = "contract C { function f() public { greet(bar(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii), cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm); } }\n";
        let guard = comment_spans(text);
        let opts = FormatOptions::default();
        let (edits, regions) = wrap_edits(text, &guard, &opts);
        assert_eq!(edits.len(), 1, "outer only: {edits:?}");
        assert_eq!(regions.spans.len(), 1);
        assert_idempotent(text);
    }

    #[test]
    fn custom_threshold_wraps_early() {
        let opts = FormatOptions {
            line_length: 40,
            ..Default::default()
        };
        let text = "contract C { greet(aaaa, bbbb, cccc, dddd); }\n";
        assert!(text.chars().count() > 40);
        let (out, regions) = wrap_with(text, &opts);
        assert!(out.contains("greet(\n"), "out: {out}");
        assert!(!regions.is_empty());
        let once = wrap_with(&out, &opts).0;
        assert_eq!(out, once);
    }

    #[test]
    fn custom_threshold_leaves_short_lines() {
        let opts = FormatOptions {
            line_length: 200,
            ..Default::default()
        };
        let text = "contract C { function f() public { greet(aaaa, bbbb); } }\n";
        let (out, regions) = wrap_with(text, &opts);
        assert_eq!(out, text);
        assert!(regions.is_empty());
    }

    #[test]
    fn tab_style_uses_tab_continuation() {
        let opts = FormatOptions {
            style: crate::options::IndentStyle::Tab,
            ..Default::default()
        };
        let text = "contract C { function f() public { greet(aaaa, bbbb, cccc, dddd, eeee, ffff, gggg, hhhh, iiii, jjjj, kkkk, llll, mmmm, nnnn, oooo, pppp); } }\n";
        let (out, _) = wrap_with(text, &opts);
        assert!(out.contains("greet(\n"), "out: {out}");
        assert!(out.contains("\n\taaaa,"), "out: {out}");
        let twice = wrap_with(&out, &opts).0;
        assert_eq!(out, twice);
    }

    #[test]
    fn wrap_regions_skip_inner_lines() {
        let opts = FormatOptions::default();
        let text = "contract C { function f() public { greet(a, b,\n c); } }\n";
        let (_, regions) = wrap_with(text, &opts);
        assert!(!regions.is_empty());
        let lines: Vec<usize> = crate::whitespace::split_lines(text)
            .iter()
            .map(|line| line.start)
            .collect();
        assert!(lines.len() >= 2);
        assert!(!regions.skips_line(lines[0]));
        assert!(regions.skips_line(lines[1]));
    }
}
