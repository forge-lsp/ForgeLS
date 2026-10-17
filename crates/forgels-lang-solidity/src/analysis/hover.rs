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
/// Array and address members use the cursor member span since their
/// detail spans are synthetic. Emit name slots resolve to event declarations only, so a
/// function with the same name never shadows the event. Positions on
/// whitespace, operators, or undeclared words return `None`.
pub fn hover_at(file: &SolidityFile, text: &str, offset: u32) -> Option<Hover> {
    // Receiver itself keeps scope hover; only the member uses type lookup.
    if let Some(ctx) = super::members::member_context_at(text, offset)
        && (ctx.member_span.contains(offset) || ctx.member_span.end == offset)
    {
        let scope = collect_symbols(file);
        let kind = super::members::receiver_type(file, &scope, offset, &ctx.receiver);
        if let super::members::TypeKind::Array { dynamic } = kind
            && let Some(found) = super::array::array_member(&ctx.member, dynamic)
        {
            return Some(Hover {
                contents: found.detail,
                span: ctx.member_span,
            });
        }
        if matches!(kind, super::members::TypeKind::Address)
            && let Some(found) = super::address::address_member(&ctx.member)
        {
            return Some(Hover {
                contents: found.detail,
                span: ctx.member_span,
            });
        }
        if let Some(found) = super::members::members_of(file, &kind)
            .into_iter()
            .find(|member| member.name == ctx.member)
        {
            return Some(Hover {
                contents: found.detail,
                span: found.span,
            });
        }
        if let Some(builtin) =
            super::forge::member_builtin(&ctx.receiver, &ctx.member, &scope, offset)
        {
            return Some(Hover {
                contents: super::forge::builtin_detail(builtin),
                span: ctx.member_span,
            });
        }
        return None;
    }
    if let Some((name, span)) = super::emits::emit_name_at(text, offset) {
        let found = super::emits::resolve_local_emit(file, offset, &name)?;
        return Some(Hover {
            contents: found.detail,
            span,
        });
    }
    let (word, span) = word_at(text, offset)?;
    let scope = collect_symbols(file);
    if let Some(symbol) = scope.resolve(&word, offset) {
        return Some(Hover {
            contents: symbol.detail.clone(),
            span: symbol.span,
        });
    }
    let builtin = super::forge::plain_builtin(file, &scope, offset, &word)?;
    Some(Hover {
        contents: super::forge::builtin_detail(builtin),
        span,
    })
}
