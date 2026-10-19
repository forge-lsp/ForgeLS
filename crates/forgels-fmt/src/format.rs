use forgels_syntax::Span;

use super::edit::{FormatOutcome, apply};
use super::trim::trim_edits;

// Solidity-only V1 entry point. Noir stays out: no second grammar is
// wired until the formatter rules settle.
pub fn format_text(text: &str) -> FormatOutcome {
    if text.is_empty() {
        return FormatOutcome {
            text: String::new(),
            applied: 0,
            skipped: 0,
        };
    }
    let guard = comment_spans(text);
    let edits = trim_edits(text, &guard);
    apply(text, &edits)
}

// Minimal comment spans (`//` to newline, `/*` to close or EOF).
// Guards trim edits only; full re-lex stays P2. Strings are skipped
// so quoted markers never count as comments.
pub(crate) fn comment_spans(text: &str) -> Vec<Span> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            let start = index;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            out.push(Span::new(start as u32, index as u32));
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let start = index;
            index += 2;
            while index < bytes.len() {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    index += 2;
                    break;
                }
                index += 1;
            }
            out.push(Span::new(start as u32, index as u32));
        } else if byte == b'"' || byte == b'\'' {
            let quote = byte;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == quote {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
        } else {
            let width = text[index..]
                .chars()
                .next()
                .map(|first| first.len_utf8())
                .unwrap_or(1);
            index += width;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_text_has_no_edits() {
        let out = format_text("contract A {}\n");
        assert_eq!(out.text, "contract A {}\n");
        assert_eq!(out.applied, 0);
    }

    #[test]
    fn trims_and_ends_with_single_newline() {
        let out = format_text("contract A {}   ");
        assert_eq!(out.text, "contract A {}\n");
    }

    #[test]
    fn comment_line_keeps_trailing_spaces() {
        let out = format_text("// hi   \n");
        assert_eq!(out.text, "// hi   \n");
    }

    #[test]
    fn code_line_still_trims() {
        let out = format_text("uint x;   \n");
        assert_eq!(out.text, "uint x;\n");
    }

    #[test]
    fn format_is_idempotent() {
        for sample in [
            "contract A {}   \n\n",
            "a;\r\nb;  \r\n",
            "a;\n   \n",
            "",
            "   \n",
            "// hi   \n",
            "uint x;\t\n",
        ] {
            let once = format_text(sample).text;
            let twice = format_text(&once).text;
            assert_eq!(once, twice, "idempotent for {sample:?}");
        }
    }

    #[test]
    fn block_comment_guards_covered_lines() {
        let text = "/* a;   \n   b; */\nuint x;   \n";
        let out = format_text(text);
        assert_eq!(out.text, "/* a;   \n   b; */\nuint x;\n");
    }

    #[test]
    fn quoted_markers_are_not_comments() {
        assert!(comment_spans("\"//\";\n").is_empty());
    }
}
