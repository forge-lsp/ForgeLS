use super::cursor::{Cursor, token_element};
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxNode};

/// Parse `struct Name { ... }`, keeping only the name.
///
/// Body is skipped brace aware. EOF without `}` still yields the
/// partial node so typing state keeps the symbol.
pub(crate) fn parse_struct(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    parse_user_type(cursor, errors, SolidityKind::Struct, SolidityNode::Struct)
}

/// Parse `enum Name { ... }`, keeping only the name.
///
/// Body is skipped brace aware. EOF without `}` still yields the
/// partial node so typing state keeps the symbol.
pub(crate) fn parse_enum(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    parse_user_type(cursor, errors, SolidityKind::Enum, SolidityNode::Enum)
}

fn parse_user_type(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
    keyword: SolidityKind,
    kind: SolidityNode,
) -> Option<SyntaxNode> {
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected type name"));
        return None;
    };
    if head.kind != keyword.as_u16() {
        errors.push(ParseError::new(head.span, "expected type name"));
        return None;
    }
    let start = head.span.start;
    let name = match cursor.expect(SolidityKind::Ident, "type name") {
        Ok(name) => name,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let name_end = name.span.end;
    let children = vec![token_element(name)];
    if cursor.peek_kind() != Some(SolidityKind::LBrace.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `{`"));
        return Some(SyntaxNode::with_children(
            kind.as_u16(),
            Span::new(start, name_end),
            children,
        ));
    }
    super::members::skip_balanced(cursor);
    let last_is_close = cursor
        .tokens
        .get(cursor.pos().saturating_sub(1))
        .is_some_and(|token| token.kind == SolidityKind::RBrace.as_u16());
    if last_is_close {
        let end = cursor
            .tokens
            .get(cursor.pos().saturating_sub(1))
            .map(|token| token.span.end)
            .unwrap_or(name_end);
        return Some(SyntaxNode::with_children(
            kind.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
    let end = cursor.peek_span().end.max(name_end);
    Some(SyntaxNode::with_children(
        kind.as_u16(),
        Span::new(start, end),
        children,
    ))
}
