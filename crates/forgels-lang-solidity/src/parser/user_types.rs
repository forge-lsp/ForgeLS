use super::cursor::{Cursor, token_element};
use super::members::parse_param;
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
