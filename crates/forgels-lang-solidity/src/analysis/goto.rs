use super::{collect_symbols, word_at};
use crate::{SolidityFile, SolidityLexer, SolidityParser};
use forgels_syntax::{Lexer, Span};

/// Jump target for the word at `offset`: the declaration span.
///
/// Positions on whitespace, operators, or undeclared words return `None`.
/// The caller attaches the document URI; the span alone locates the jump.
pub fn goto_definition(file: &SolidityFile, text: &str, offset: u32) -> Option<Span> {
    let (word, _) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    scope.resolve(&word, offset).map(|symbol| symbol.span)
}

/// Exported declaration span for `word` inside a dependency text.
///
/// Parses with recovery so broken deps still resolve. Only contracts,
/// functions, structs, and enums cross files; locals, params, and
/// state vars stay local.
pub fn resolve_in_dep(dep_text: &str, word: &str) -> Option<Span> {
    let tokens = SolidityLexer.tokenize(dep_text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let scope = collect_symbols(&file);
    scope
        .export_symbols()
        .iter()
        .find(|symbol| symbol.name == word)
        .map(|symbol| symbol.span)
}
