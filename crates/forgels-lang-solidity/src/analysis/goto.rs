use super::{collect_symbols, word_at};
use crate::{SolidityFile, SolidityLexer, SolidityParser};
use forgels_syntax::{Lexer, Span};

/// Jump target for the word at `offset`: the declaration span.
///
/// Member positions after a dot jump to the field or contract member.
/// Array members (`length`, `push`, `pop`) and address members
/// (`balance`, `transfer`, and rest) return `None` by design: their
/// spans are synthetic with no declaration in the workspace. Emit name
/// slots jump to the event declaration only. Positions on whitespace,
/// operators, or undeclared words return `None`.
/// Forge builtins (`vm.*`, `console.*`, `Test` asserts) return `None` by
/// design: they have no declaration in the workspace. The caller guards
/// dep lookup the same way so builtins never jump to coincidental matches.
/// The caller attaches the document URI; the span alone locates the jump.
pub fn goto_definition(file: &SolidityFile, text: &str, offset: u32) -> Option<Span> {
    if let Some(ctx) = super::members::member_context_at(text, offset)
        && (ctx.member_span.contains(offset) || ctx.member_span.end == offset)
    {
        let scope = collect_symbols(file);
        let kind = super::members::receiver_type(file, &scope, offset, &ctx.receiver);
        if matches!(
            kind,
            super::members::TypeKind::Array { .. } | super::members::TypeKind::Address
        ) {
            return None;
        }
        let found = super::members::members_of(file, &kind)
            .into_iter()
            .find(|member| member.name == ctx.member)?;
        return Some(found.span);
    }
    if let Some((name, _)) = super::emits::emit_name_at(text, offset) {
        let found = super::emits::resolve_local_emit(file, offset, &name)?;
        return Some(found.span);
    }
    let (word, _) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    scope.resolve(&word, offset).map(|symbol| symbol.span)
}

/// Exported declaration span for `word` inside a dependency text.
///
/// Parses with recovery so broken deps still resolve. Only exported
/// symbols cross files: contracts, functions, structs, enums, errors,
/// events, modifiers, and user types. Locals, params, and state vars
/// stay local.
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
