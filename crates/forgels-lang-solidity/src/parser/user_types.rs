use super::cursor::{Cursor, token_element};
use super::members::{parse_param, skip_parens};
use super::types::parse_type;
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode};

/// Parse `struct Name { Type name; ... }`, keeping name plus fields.
///
/// Fields reuse param parsing for type plus data location. Broken fields
/// push one error and skip to `;` or `}` so siblings still parse. EOF
/// without `}` still yields the partial node for typing state.
pub(crate) fn parse_struct(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let (start, name, mut children) = parse_type_header(cursor, errors, SolidityKind::Struct)?;
    if cursor.peek_kind() != Some(SolidityKind::LBrace.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `{`"));
        let end = name.span.end;
        return Some(SyntaxNode::with_children(
            SolidityNode::Struct.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    cursor.bump();
    loop {
        match cursor.peek_kind() {
            None => {
                errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
                let end = cursor.peek_span().end.max(name.span.end);
                return Some(SyntaxNode::with_children(
                    SolidityNode::Struct.as_u16(),
                    Span::new(start, end),
                    children,
                ));
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => {
                let Some(close) = cursor.bump() else {
                    errors.push(ParseError::new(cursor.peek_span(), "expected `}`"));
                    let end = cursor.peek_span().end.max(name.span.end);
                    return Some(SyntaxNode::with_children(
                        SolidityNode::Struct.as_u16(),
                        Span::new(start, end),
                        children,
                    ));
                };
                return Some(SyntaxNode::with_children(
                    SolidityNode::Struct.as_u16(),
                    Span::new(start, close.span.end),
                    children,
                ));
            }
            _ => {
                if let Some(field) = parse_struct_field(cursor, errors) {
                    children.push(SyntaxElement::Node(field));
                }
            }
        }
    }
}

/// Parse `enum Name { A, B }`, keeping name plus variant names.
///
/// Variants are cheap ident tokens. Broken rows push one error and move
/// forward so siblings still parse.
pub(crate) fn parse_enum(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let (start, name, mut children) = parse_type_header(cursor, errors, SolidityKind::Enum)?;
    if cursor.peek_kind() != Some(SolidityKind::LBrace.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `{`"));
        let end = name.span.end;
        return Some(SyntaxNode::with_children(
            SolidityNode::Enum.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    cursor.bump();
    loop {
        match cursor.peek_kind() {
            None => {
                errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
                let end = cursor.peek_span().end.max(name.span.end);
                return Some(SyntaxNode::with_children(
                    SolidityNode::Enum.as_u16(),
                    Span::new(start, end),
                    children,
                ));
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => {
                let Some(close) = cursor.bump() else {
                    errors.push(ParseError::new(cursor.peek_span(), "expected `}`"));
                    let end = cursor.peek_span().end.max(name.span.end);
                    return Some(SyntaxNode::with_children(
                        SolidityNode::Enum.as_u16(),
                        Span::new(start, end),
                        children,
                    ));
                };
                return Some(SyntaxNode::with_children(
                    SolidityNode::Enum.as_u16(),
                    Span::new(start, close.span.end),
                    children,
                ));
            }
            Some(kind) if kind == SolidityKind::Ident.as_u16() => {
                let Some(variant) = cursor.bump() else {
                    errors.push(ParseError::new(cursor.peek_span(), "expected variant name"));
                    return Some(SyntaxNode::with_children(
                        SolidityNode::Enum.as_u16(),
                        Span::new(start, cursor.peek_span().end.max(name.span.end)),
                        children,
                    ));
                };
                children.push(token_element(variant));
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    cursor.bump();
                } else if cursor.peek_kind() != Some(SolidityKind::RBrace.as_u16())
                    && cursor.peek_kind().is_some()
                {
                    // Missing comma between variants: one error, keep both.
                    if cursor.peek_kind() == Some(SolidityKind::Ident.as_u16()) {
                        errors.push(ParseError::new(cursor.peek_span(), "expected `,`"));
                    }
                }
            }
            Some(_) => {
                errors.push(ParseError::new(cursor.peek_span(), "expected variant name"));
                cursor.bump();
            }
        }
    }
}

fn parse_type_header(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
    keyword: SolidityKind,
) -> Option<(u32, forgels_syntax::Token, Vec<SyntaxElement>)> {
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
    let children = vec![token_element(name.clone())];
    Some((start, name, children))
}

fn parse_struct_field(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    use super::types::is_type_start;
    let peek = cursor.peek_kind();
    let is_field_start = matches!(peek, Some(kind)
        if is_type_start(kind) || kind == SolidityKind::Ident.as_u16());
    if !is_field_start {
        errors.push(ParseError::new(cursor.peek_span(), "expected type"));
        cursor.bump();
        recover_field(cursor);
        return None;
    }
    let field = match parse_param(cursor) {
        Ok(field) => field,
        Err(error) => {
            errors.push(error);
            recover_field(cursor);
            return None;
        }
    };
    let has_name = field_has_name(&field);
    if !has_name {
        errors.push(ParseError::new(field.span, "expected field name"));
        if cursor.peek_kind() == Some(SolidityKind::Semicolon.as_u16()) {
            cursor.bump();
        } else {
            recover_field(cursor);
        }
        return None;
    }
    if cursor.peek_kind() != Some(SolidityKind::Semicolon.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
        return Some(field);
    }
    cursor.bump();
    Some(field)
}

fn field_has_name(field: &SyntaxNode) -> bool {
    field.children.iter().any(|element| match element {
        SyntaxElement::Token(token) => token.kind == SolidityKind::Ident.as_u16(),
        _ => false,
    })
}

fn recover_field(cursor: &mut Cursor) {
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::Semicolon.as_u16() {
            cursor.bump();
            return;
        }
        if kind == SolidityKind::RBrace.as_u16() {
            return;
        }
        cursor.bump();
        if cursor.at_end() {
            return;
        }
    }
}

/// Parse `error Name(params);`, keeping name plus params.
///
/// Params reuse param parsing so arity checks count them. Missing
/// `;` is one error with the node kept for hover and goto.
pub(crate) fn parse_error(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected error name"));
        return None;
    };
    if head.kind != SolidityKind::ErrorKw.as_u16() {
        errors.push(ParseError::new(head.span, "expected error name"));
        return None;
    }
    let start = head.span.start;
    let name = match cursor.expect(SolidityKind::Ident, "error name") {
        Ok(name) => name,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let mut node_children = vec![token_element(name.clone())];
    if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
        cursor.bump();
        if cursor.peek_kind() != Some(SolidityKind::RParen.as_u16()) {
            loop {
                match parse_param(cursor) {
                    Ok(param) => node_children.push(SyntaxElement::Node(param)),
                    Err(error) => {
                        errors.push(error);
                        return None;
                    }
                }
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    cursor.bump();
                } else {
                    break;
                }
            }
        }
        if cursor.expect(SolidityKind::RParen, "`)`").is_err() {
            errors.push(ParseError::new(cursor.peek_span(), "expected `)`"));
            return None;
        }
    }
    let end = match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => semi.span.end,
        Err(error) => {
            errors.push(error);
            name.span.end
        }
    };
    Some(SyntaxNode::with_children(
        SolidityNode::Error.as_u16(),
        Span::new(start, end),
        node_children,
    ))
}

/// Parse `type Name is uint;`, keeping name plus underlying type.
///
/// Only the `is Type ;` shape is supported. Anything else is one
/// error with no node so the file keeps parsing.
pub(crate) fn parse_user_type(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected type name"));
        return None;
    };
    if head.kind != SolidityKind::Type.as_u16() {
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
    if cursor.peek_kind() != Some(SolidityKind::Is.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `is`"));
        return None;
    }
    cursor.bump();
    let underlying = match parse_type(cursor) {
        Ok(node) => node,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let end = match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => semi.span.end,
        Err(error) => {
            errors.push(error);
            underlying.span.end.max(name.span.end)
        }
    };
    Some(SyntaxNode::with_children(
        SolidityNode::UserType.as_u16(),
        Span::new(start, end),
        vec![token_element(name), SyntaxElement::Node(underlying)],
    ))
}

/// Parse `event Name(params) [anonymous];`, keeping name plus params.
///
/// Params reuse param parsing so `indexed` stays attached to the name.
/// Missing `;` is one error with the node kept for hover and goto.
pub(crate) fn parse_event(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected event name"));
        return None;
    };
    if head.kind != SolidityKind::Event.as_u16() {
        errors.push(ParseError::new(head.span, "expected event name"));
        return None;
    }
    let start = head.span.start;
    let name = match cursor.expect(SolidityKind::Ident, "event name") {
        Ok(name) => name,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let mut children = vec![token_element(name.clone())];
    if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
        cursor.bump();
        if cursor.peek_kind() != Some(SolidityKind::RParen.as_u16()) {
            loop {
                match parse_param(cursor) {
                    Ok(param) => children.push(SyntaxElement::Node(param)),
                    Err(error) => {
                        errors.push(error);
                        return None;
                    }
                }
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    cursor.bump();
                } else {
                    break;
                }
            }
        }
        if cursor.expect(SolidityKind::RParen, "`)`").is_err() {
            errors.push(ParseError::new(cursor.peek_span(), "expected `)`"));
            return None;
        }
    }
    if cursor.peek_kind() == Some(SolidityKind::Anonymous.as_u16()) {
        cursor.bump();
    }
    let end = match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => semi.span.end,
        Err(error) => {
            errors.push(error);
            name.span.end
        }
    };
    Some(SyntaxNode::with_children(
        SolidityNode::Event.as_u16(),
        Span::new(start, end),
        children,
    ))
}

/// Parse `modifier Name(params) ... { body }`, keeping the name only.
///
/// Params are skipped with paren balance and the body with brace
/// balance so malformed rows still yield the name. A `;` stub is also
/// accepted for forward declarations.
pub(crate) fn parse_modifier(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    use super::members::skip_balanced;
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(
            cursor.peek_span(),
            "expected modifier name",
        ));
        return None;
    };
    if head.kind != SolidityKind::Modifier.as_u16() {
        errors.push(ParseError::new(head.span, "expected modifier name"));
        return None;
    }
    let start = head.span.start;
    let name = match cursor.expect(SolidityKind::Ident, "modifier name") {
        Ok(name) => name,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
        skip_parens(cursor);
    }
    // Optional modifiers on the modifier itself: `virtual`, `override`,
    // or custom names. Consumed, not stored.
    loop {
        match cursor.peek_kind() {
            Some(kind)
                if kind == SolidityKind::Virtual.as_u16()
                    || kind == SolidityKind::Override.as_u16() =>
            {
                cursor.bump();
                if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
                    skip_parens(cursor);
                }
            }
            Some(kind) if kind == SolidityKind::Ident.as_u16() => {
                cursor.bump();
                if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
                    skip_parens(cursor);
                }
            }
            _ => break,
        }
    }
    let end = if cursor.peek_kind() == Some(SolidityKind::Semicolon.as_u16()) {
        cursor
            .bump()
            .map(|semi| semi.span.end)
            .unwrap_or(name.span.end)
    } else if cursor.peek_kind() == Some(SolidityKind::LBrace.as_u16()) {
        let body_start = cursor.peek_span().start;
        skip_balanced(cursor);
        cursor
            .tokens
            .get(cursor.pos().saturating_sub(1))
            .map(|token| token.span.end)
            .unwrap_or(body_start)
    } else {
        errors.push(ParseError::new(cursor.peek_span(), "expected `{`"));
        return Some(SyntaxNode::with_children(
            SolidityNode::Modifier.as_u16(),
            Span::new(start, name.span.end),
            vec![token_element(name)],
        ));
    };
    Some(SyntaxNode::with_children(
        SolidityNode::Modifier.as_u16(),
        Span::new(start, end.max(name.span.end)),
        vec![token_element(name)],
    ))
}
