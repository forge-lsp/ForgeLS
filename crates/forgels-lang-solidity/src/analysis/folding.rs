use forgels_core::{FoldingRangeData, MAX_FOLDING_RANGES};
use forgels_syntax::{AstNode, Span};

use crate::SolidityFile;

const CAP: usize = MAX_FOLDING_RANGES;

fn line_of(text: &str, offset: u32) -> u32 {
    let end = (offset as usize).min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count() as u32
}

fn push_span(out: &mut Vec<Span>, span: Span) {
    if span.start >= span.end {
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

fn push_blocks(block: &crate::BlockNode, out: &mut Vec<Span>) {
    push_span(out, block.syntax().span);
    for child in block.child_blocks() {
        push_blocks(&child, out);
    }
}

/// Folding ranges for contracts, functions, and blocks.
///
/// Collects nested spans in source order, drops single line spans via
/// a newline count, dedupes exact duplicates, and caps at
/// `MAX_FOLDING_RANGES`. Dispatch maps each span to `startLine` and
/// `endLine`.
pub fn folding_ranges_in(file: &SolidityFile, text: &str) -> Vec<FoldingRangeData> {
    let mut spans: Vec<Span> = Vec::new();
    if text.is_empty() {
        return Vec::new();
    }
    for contract in file.contracts() {
        push_span(&mut spans, contract.syntax().span);
        for function in contract.functions() {
            push_span(&mut spans, function.syntax().span);
            if let Some(body) = function.body() {
                push_blocks(&body, &mut spans);
            }
        }
    }
    spans.retain(|span| {
        if text.get(span.start as usize..span.end as usize).is_none() {
            return false;
        }
        line_of(text, span.start) < line_of(text, span.end)
    });
    spans.sort_by(|left, right| left.start.cmp(&right.start).then(right.end.cmp(&left.end)));
    spans.truncate(CAP);
    spans.into_iter().map(FoldingRangeData::new).collect()
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
    fn nests_contract_function_and_block() {
        let text = "contract C {\nfunction f() public {\nuint x = 1;\nx;\n}\n}";
        let file = parse(text);
        let found = folding_ranges_in(&file, text);
        assert!(found.len() >= 3);
        let starts: Vec<u32> = found.iter().map(|entry| entry.span.start).collect();
        assert!(starts.contains(&0));
    }

    #[test]
    fn single_line_spans_drop() {
        let text = "contract C {}";
        let file = parse(text);
        assert!(folding_ranges_in(&file, text).is_empty());
    }

    #[test]
    fn empty_text_returns_empty() {
        let file = parse("contract C {}");
        assert!(folding_ranges_in(&file, "").is_empty());
    }
}
