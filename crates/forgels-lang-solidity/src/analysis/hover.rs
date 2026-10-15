use super::{collect_symbols, word_at};
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
