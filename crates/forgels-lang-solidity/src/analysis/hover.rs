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
/// Member positions after a dot resolve against the receiver type first.
/// Emit name slots resolve to event declarations only, so a function
/// with the same name never shadows the event. Positions on whitespace,
/// operators, or undeclared words return `None`.
pub fn hover_at(file: &SolidityFile, text: &str, offset: u32) -> Option<Hover> {
    // Receiver itself keeps scope hover; only the member uses type lookup.
    if let Some(ctx) = super::members::member_context_at(text, offset)
        && (ctx.member_span.contains(offset) || ctx.member_span.end == offset)
    {
        let scope = collect_symbols(file);
        let kind = super::members::receiver_type(file, &scope, offset, &ctx.receiver);
        let found = super::members::members_of(file, &kind)
            .into_iter()
            .find(|member| member.name == ctx.member)?;
        return Some(Hover {
            contents: found.detail,
            span: found.span,
        });
    }
    if let Some((name, span)) = super::emits::emit_name_at(text, offset) {
        let found = super::emits::resolve_local_emit(file, offset, &name)?;
        return Some(Hover {
            contents: found.detail,
            span,
        });
    }
    let (word, _) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    let symbol = scope.resolve(&word, offset)?;
    Some(Hover {
        contents: symbol.detail.clone(),
        span: symbol.span,
    })
}
