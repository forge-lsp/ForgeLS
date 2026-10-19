use forgels_syntax::Span;

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
}
