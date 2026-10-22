use forgels_syntax::Span;

use super::edit::FormatEdit;

// One source line with byte offsets. `content_end` excludes the
// newline (`\n` or `\r\n`); `end` includes it. CRLF bytes stay intact
// so trims never split `\r\n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSpan {
    pub start: usize,
    pub content_end: usize,
    pub end: usize,
}

// Split text into lines. Empty text yields no lines.
pub fn split_lines(text: &str) -> Vec<LineSpan> {
    if text.is_empty() {
        return Vec::new();
    }
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            let mut content_end = index;
            if content_end > start && bytes[content_end - 1] == b'\r' {
                content_end -= 1;
            }
            out.push(LineSpan {
                start,
                content_end,
                end: index + 1,
            });
            start = index + 1;
        }
    }
    if start < bytes.len() {
        out.push(LineSpan {
            start,
            content_end: bytes.len(),
            end: bytes.len(),
        });
    }
    out
}

// True when the line body holds only spaces, tabs, or nothing.
pub fn is_blank_gap(content: &str) -> bool {
    content.bytes().all(|byte| byte == b' ' || byte == b'\t')
}

// Trailing spaces and tabs of one line body. Returns `None` when the
// body ends with other text.
pub fn trailing_span(text: &str, start: usize, content_end: usize) -> Option<Span> {
    if text.is_empty() || start >= content_end {
        return None;
    }
    let body = text.get(start..content_end)?;
    let trimmed = body.trim_end_matches([' ', '\t']);
    let edge = start + trimmed.len();
    if edge >= content_end {
        return None;
    }
    Some(Span::new(edge as u32, content_end as u32))
}

// Token-gap spacing pass. Scans raw text into tokens plus gaps and
// normalizes gaps only. Strings and comments are tokens, so edits
// never land inside them. CRLF: same-line gaps touch spaces and tabs
// only; newline gaps stay intact except brace joins which use one
// space. Assignment family only gets spaces; other ops stay untouched.
pub fn space_edits(text: &str, guard: &[Span]) -> Vec<FormatEdit> {
    if text.is_empty() {
        return Vec::new();
    }
    let tokens = scan_gap_tokens(text);
    if tokens.len() < 2 {
        return Vec::new();
    }
    let mut edits = Vec::new();
    for pair in tokens.windows(2) {
        let prev = &pair[0];
        let next = &pair[1];
        if prev.kind == RawKind::Comment || next.kind == RawKind::Comment {
            continue;
        }
        let gap_start = prev.span.end as usize;
        let gap_end = next.span.start as usize;
        if gap_start > gap_end || gap_end > text.len() {
            continue;
        }
        let Some(gap) = text.get(gap_start..gap_end) else {
            continue;
        };
        let Some(desired) = gap_desired(prev, next, gap) else {
            continue;
        };
        if gap == desired {
            continue;
        }
        let span = Span::new(gap_start as u32, gap_end as u32);
        if guard.iter().any(|other| overlaps(span, *other)) {
            continue;
        }
        edits.push(FormatEdit::new(span, desired));
    }
    edits
}

fn overlaps(left: Span, right: Span) -> bool {
    left.start <= right.end && left.end >= right.start
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RawKind {
    Word,
    Symbol,
    StringLit,
    Comment,
    Other,
}

#[derive(Debug, Clone)]
struct RawToken {
    span: Span,
    text: String,
    kind: RawKind,
}

fn is_word_token(token: &RawToken, value: &str) -> bool {
    token.kind == RawKind::Word && token.text == value
}

fn is_symbol_token(token: &RawToken, value: &str) -> bool {
    token.kind == RawKind::Symbol && token.text == value
}

fn is_assign_symbol(text: &str) -> bool {
    matches!(
        text,
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" | ">>>="
    )
}

fn has_line_break(gap: &str) -> bool {
    gap.contains('\n') || gap.contains('\r')
}

// Desired gap text for one token pair, or `None` to leave as-is.
fn gap_desired(prev: &RawToken, next: &RawToken, gap: &str) -> Option<String> {
    // Brace joins win over generic spacing.
    if is_symbol_token(prev, "}") && is_word_token(next, "else") {
        return Some(" ".to_string());
    }
    if is_word_token(prev, "else") && is_symbol_token(next, "{") {
        return Some(" ".to_string());
    }
    if is_symbol_token(prev, ")") && is_symbol_token(next, "{") {
        return Some(" ".to_string());
    }
    // Glued keywords: no space before `(`.
    if prev.kind == RawKind::Word
        && next.kind == RawKind::Symbol
        && next.text == "("
        && matches!(
            prev.text.as_str(),
            "receive" | "fallback" | "mapping" | "type"
        )
    {
        if gap.is_empty() {
            return None;
        }
        return Some(String::new());
    }
    // Control keywords: exactly one space before `(`.
    if prev.kind == RawKind::Word
        && next.kind == RawKind::Symbol
        && next.text == "("
        && matches!(prev.text.as_str(), "if" | "for" | "while")
    {
        return Some(" ".to_string());
    }
    let break_in_gap = has_line_break(gap);
    // Zero space after open.
    if is_symbol_token(prev, "(") || is_symbol_token(prev, "[") {
        if break_in_gap {
            return None;
        }
        if gap.is_empty() {
            return None;
        }
        return Some(String::new());
    }
    // Zero space before close, comma, semicolon.
    if is_symbol_token(next, ")")
        || is_symbol_token(next, "]")
        || is_symbol_token(next, ",")
        || is_symbol_token(next, ";")
    {
        if break_in_gap {
            return None;
        }
        if gap.is_empty() {
            return None;
        }
        return Some(String::new());
    }
    // Single space after comma unless closing, newline, or EOF.
    if is_symbol_token(prev, ",") {
        if break_in_gap {
            return None;
        }
        if is_symbol_token(next, ")") || is_symbol_token(next, "]") || is_symbol_token(next, "}") {
            if gap.is_empty() {
                return None;
            }
            return Some(String::new());
        }
        return Some(" ".to_string());
    }
    // Single space after semicolon unless paren or bracket close or newline.
    // A `}` keeps one space (`; }`) for readability.
    if is_symbol_token(prev, ";") {
        if break_in_gap {
            return None;
        }
        if is_symbol_token(next, ")") || is_symbol_token(next, "]") {
            if gap.is_empty() {
                return None;
            }
            return Some(String::new());
        }
        return Some(" ".to_string());
    }
    // Assignment family only: single spaces both sides.
    if (prev.kind == RawKind::Symbol && is_assign_symbol(&prev.text))
        || (next.kind == RawKind::Symbol && is_assign_symbol(&next.text))
    {
        if break_in_gap {
            return None;
        }
        return Some(" ".to_string());
    }
    // Power operator stays glued with no spaces.
    if (prev.kind == RawKind::Symbol && prev.text == "**")
        || (next.kind == RawKind::Symbol && next.text == "**")
    {
        if break_in_gap {
            return None;
        }
        if gap.is_empty() {
            return None;
        }
        return Some(String::new());
    }
    None
}

fn scan_gap_tokens(text: &str) -> Vec<RawToken> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let byte = bytes[pos];
        if byte == b' ' || byte == b'\t' || byte == b'\n' || byte == b'\r' {
            pos += 1;
            continue;
        }
        if byte == b'/' && bytes.get(pos + 1) == Some(&b'/') {
            let start = pos;
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            push_token(text, &mut out, start, pos, RawKind::Comment);
            continue;
        }
        if byte == b'/' && bytes.get(pos + 1) == Some(&b'*') {
            let start = pos;
            pos += 2;
            while pos < bytes.len() {
                if bytes[pos] == b'*' && bytes.get(pos + 1) == Some(&b'/') {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            push_token(text, &mut out, start, pos, RawKind::Comment);
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            let start = pos;
            let quote = byte;
            pos += 1;
            while pos < bytes.len() {
                if bytes[pos] == b'\\' {
                    pos = (pos + 2).min(bytes.len());
                } else if bytes[pos] == quote {
                    pos += 1;
                    break;
                } else {
                    pos += 1;
                }
            }
            push_token(text, &mut out, start, pos, RawKind::StringLit);
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' {
            let start = pos;
            while pos < bytes.len()
                && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_' || bytes[pos] == b'$')
            {
                pos += 1;
            }
            push_token(text, &mut out, start, pos, RawKind::Word);
            continue;
        }
        if byte.is_ascii_digit() {
            let start = pos;
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
                pos += 1;
            }
            push_token(text, &mut out, start, pos, RawKind::Word);
            continue;
        }
        if let Some(len) = symbol_len_at(text, pos) {
            push_token(text, &mut out, pos, pos + len, RawKind::Symbol);
            pos += len;
            continue;
        }
        let width = text[pos..]
            .chars()
            .next()
            .map(|first| first.len_utf8())
            .unwrap_or(1);
        push_token(text, &mut out, pos, pos + width, RawKind::Other);
        pos += width;
    }
    out
}

fn push_token(text: &str, out: &mut Vec<RawToken>, start: usize, end: usize, kind: RawKind) {
    let Some(slice) = text.get(start..end) else {
        return;
    };
    out.push(RawToken {
        span: Span::new(start as u32, end as u32),
        text: slice.to_string(),
        kind,
    });
}

fn symbol_len_at(text: &str, pos: usize) -> Option<usize> {
    const SYMBOLS: &[&str] = &[
        ">>>=", ">>>", ">>=", "<<=", ">>", "<<", "**", "+=", "-=", "*=", "/=", "%=", "&=", "|=",
        "^=", "==", "!=", "<=", ">=", "&&", "||", "=>", "->", "++", "--", "(", ")", "[", "]", "{",
        "}", ";", ",", ":", ".", "=", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^", "~", "!",
        "?",
    ];
    let rest = text.get(pos..)?;
    for symbol in SYMBOLS {
        if rest.starts_with(symbol) {
            return Some(symbol.len());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_has_no_lines() {
        assert!(split_lines("").is_empty());
    }

    #[test]
    fn lf_and_crlf_split_without_splitting_cr() {
        let lines = split_lines("ab\r\ncd\n");
        assert_eq!(
            lines,
            vec![
                LineSpan {
                    start: 0,
                    content_end: 2,
                    end: 4,
                },
                LineSpan {
                    start: 4,
                    content_end: 6,
                    end: 7,
                },
            ]
        );
    }

    #[test]
    fn unterminated_tail_is_one_line() {
        let lines = split_lines("ab\ncd");
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[1],
            LineSpan {
                start: 3,
                content_end: 5,
                end: 5,
            }
        );
    }

    #[test]
    fn blank_gap_covers_empty_spaces_and_tabs() {
        assert!(is_blank_gap(""));
        assert!(is_blank_gap("   "));
        assert!(is_blank_gap(" \t "));
        assert!(!is_blank_gap(" x "));
    }

    #[test]
    fn trailing_span_finds_spaces_and_tabs_only() {
        let text = "ab \t";
        assert_eq!(trailing_span(text, 0, 4), Some(Span::new(2, 4)));
        assert_eq!(trailing_span("ab", 0, 2), None);
        assert_eq!(trailing_span("", 0, 0), None);
    }

    fn space_one(text: &str) -> String {
        let edits = space_edits(text, &[]);
        crate::edit::apply(text, &edits).text
    }

    fn assert_space_idempotent(text: &str) {
        let once = space_one(text);
        let twice = space_one(&once);
        assert_eq!(once, twice, "space idempotent for {text:?}");
    }

    #[test]
    fn paren_bracket_gaps_are_glued() {
        assert_eq!(space_one("foo( a )"), "foo(a)");
        assert_eq!(space_one("a[ 1 ]"), "a[1]");
        assert_eq!(space_one("foo( a,  b )"), "foo(a, b)");
        assert_space_idempotent("foo( a,  b )");
    }

    #[test]
    fn comma_semicolon_spacing_is_single() {
        assert_eq!(space_one("foo(a,b)"), "foo(a, b)");
        assert_eq!(space_one("foo(a ,b)"), "foo(a, b)");
        assert_eq!(space_one("foo(a,  b)"), "foo(a, b)");
        assert_eq!(space_one("a;b"), "a; b");
        assert_eq!(space_one("a ;b"), "a; b");
        assert_space_idempotent("foo(a,b)");
    }

    #[test]
    fn assignment_family_gets_single_spaces() {
        assert_eq!(space_one("a=1"), "a = 1");
        assert_eq!(space_one("a  =  1"), "a = 1");
        assert_eq!(space_one("a+=1"), "a += 1");
        assert_eq!(space_one("a<<=1"), "a <<= 1");
        assert_space_idempotent("a=1");
    }

    #[test]
    fn equality_and_other_ops_stay_untouched() {
        assert_eq!(space_one("a==b"), "a==b");
        assert_eq!(space_one("a +b"), "a +b");
        assert_eq!(space_one("a  +  b"), "a  +  b");
    }

    #[test]
    fn power_operator_stays_glued() {
        assert_eq!(space_one("a ** b"), "a**b");
        assert_eq!(space_one("a**b"), "a**b");
        assert_eq!(space_one("a  **  b"), "a**b");
        assert_space_idempotent("a ** b");
    }

    #[test]
    fn control_keywords_take_single_space() {
        assert_eq!(space_one("if(a)"), "if (a)");
        assert_eq!(space_one("if  (a)"), "if (a)");
        assert_eq!(space_one("for(a;b;c)"), "for (a; b; c)");
        assert_eq!(space_one("while(a)"), "while (a)");
        assert_space_idempotent("if(a)");
    }

    #[test]
    fn brace_joins_share_one_line() {
        assert_eq!(space_one("if (a)\n{ }"), "if (a) { }");
        assert_eq!(space_one("if (a){ }"), "if (a) { }");
        assert_eq!(space_one("}else {"), "} else {");
        assert_eq!(space_one("}\nelse\n{ }"), "} else { }");
        assert_space_idempotent("if (a)\n{ }");
    }

    #[test]
    fn glued_keywords_keep_no_space() {
        assert_eq!(space_one("receive ()"), "receive()");
        assert_eq!(space_one("fallback ()"), "fallback()");
        assert_eq!(
            space_one("mapping (address => uint)"),
            "mapping(address => uint)"
        );
        assert_eq!(space_one("type (uint).max"), "type(uint).max");
        assert_space_idempotent("receive ()");
    }

    #[test]
    fn strings_and_comments_block_spacing() {
        assert_eq!(space_one("\"a,  b\""), "\"a,  b\"");
        assert_eq!(space_one("foo(a /* c */ ,b)"), "foo(a /* c */ , b)");
        assert_space_idempotent("\"a,  b\"");
    }

    #[test]
    fn wrap_newlines_survive_spacing() {
        let text = "foo(\n    a,\n    b\n)";
        assert_eq!(space_one(text), text);
    }
}
