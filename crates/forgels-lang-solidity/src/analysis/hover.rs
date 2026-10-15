use super::{collect_symbols, is_word_byte};
use crate::SolidityFile;
use forgels_syntax::Span;

/// Hover result: display text plus the declaration span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    pub contents: String,
    pub span: Span,
}

/// Hover info for the word at `offset`, if it names a visible declaration.
///
/// Positions on whitespace, operators, or undeclared words return `None`.
pub fn hover_at(file: &SolidityFile, text: &str, offset: u32) -> Option<Hover> {
    let (word, _) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    let symbol = scope.resolve(&word, offset)?;
    Some(Hover {
        contents: symbol.detail.clone(),
        span: symbol.span,
    })
}

/// Word under the cursor: longest ident-char run containing `offset`.
fn word_at(text: &str, offset: u32) -> Option<(String, Span)> {
    let bytes = text.as_bytes();
    let index = offset as usize;
    if index >= bytes.len() || !is_word_byte(bytes[index]) {
        return None;
    }
    let mut start = index;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = index;
    while end < bytes.len() && is_word_byte(bytes[end]) {
        end += 1;
    }
    text.get(start..end)
        .map(|word| (word.to_string(), Span::new(start as u32, end as u32)))
}
