use forgels_core::MAX_SELECTION_DEPTH;
use forgels_syntax::{AstNode, Span};

use super::word_at;
use crate::SolidityFile;

const CAP: usize = MAX_SELECTION_DEPTH;

fn push_span(text: &str, out: &mut Vec<Span>, span: Span) {
    if text.get(span.start as usize..span.end as usize).is_none() {
        return;
    }
    if span.start > span.end {
        return;
    }
    if out
        .iter()
        .any(|known: &Span| known.start == span.start && known.end == span.end)
    {
        return;
    }
    out.push(span);
}

fn push_block_chain(block: &crate::BlockNode, offset: u32, out: &mut Vec<Span>, text: &str) {
    for child in block.child_blocks() {
        let span = child.syntax().span;
        if span.contains(offset) || span.end == offset {
            push_block_chain(&child, offset, out, text);
        }
    }
    push_span(text, out, block.syntax().span);
}

/// Selection chain at `offset`, innermost first.
///
/// Starts with the word under the cursor, then the innermost block
/// out to the function body, then the function, the enclosing
/// contract, and the file span. Dedupes exact duplicates and caps at
/// `MAX_SELECTION_DEPTH`. Dispatch nests the list into `parent` links.
pub fn selection_chain_in(file: &SolidityFile, text: &str, offset: u32) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    if text.is_empty() {
        return out;
    }
    let clamped = offset.min(text.len() as u32);
    if let Some((_, word)) = word_at(text, clamped) {
        push_span(text, &mut out, word);
    }
    for contract in file.contracts() {
        let contract_span = contract.syntax().span;
        if !contract_span.contains(clamped) && contract_span.end != clamped {
            continue;
        }
        for function in contract.functions() {
            let function_span = function.syntax().span;
            if !function_span.contains(clamped) && function_span.end != clamped {
                continue;
            }
            if let Some(body) = function.body() {
                let body_span = body.syntax().span;
                if body_span.contains(clamped) || body_span.end == clamped {
                    push_block_chain(&body, clamped, &mut out, text);
                }
            }
            push_span(text, &mut out, function_span);
        }
        push_span(text, &mut out, contract_span);
    }
    push_span(text, &mut out, file.syntax().span);
    out.truncate(CAP);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgels_syntax::Lexer;

    fn parse(text: &str) -> SolidityFile {
        let tokens = crate::SolidityLexer.tokenize(text);
        let (file, _) = crate::SolidityParser.parse_with_errors(&tokens);
        file
    }

    #[test]
    fn chain_nests_word_to_file() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\n}\n}";
        let file = parse(text);
        let at = text.find("x = 1").expect("local") as u32;
        let found = selection_chain_in(&file, text, at);
        assert!(found.len() >= 4);
        assert!(found[0].len() <= found[found.len() - 1].len());
        let covered = &text[found[0].start as usize..found[0].end as usize];
        assert_eq!(covered, "x");
    }

    #[test]
    fn depth_caps_at_max() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\n}\n}";
        let file = parse(text);
        let at = text.find("x = 1").expect("local") as u32;
        assert!(selection_chain_in(&file, text, at).len() <= MAX_SELECTION_DEPTH);
    }

    #[test]
    fn empty_text_returns_empty() {
        let file = parse("contract C {}");
        assert!(selection_chain_in(&file, "", 0).is_empty());
    }
}
