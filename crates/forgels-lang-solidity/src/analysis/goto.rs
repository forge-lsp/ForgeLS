use super::{collect_symbols, word_at};
use crate::SolidityFile;
use forgels_syntax::Span;

/// Jump target for the word at `offset`: the declaration span.
///
/// Positions on whitespace, operators, or undeclared words return `None`.
/// The caller attaches the document URI; the span alone locates the jump.
pub fn goto_definition(file: &SolidityFile, text: &str, offset: u32) -> Option<Span> {
    let (word, _) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    scope.resolve(&word, offset).map(|symbol| symbol.span)
}
