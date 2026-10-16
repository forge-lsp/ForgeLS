use super::cursor::{Cursor, token_element};
use super::members::{parse_function, parse_state_var, skip_balanced, skip_parens};
use super::types::is_type_start;
use super::user_types::{
    parse_enum, parse_error, parse_event, parse_modifier, parse_struct, parse_user_type,
};
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode};

pub(crate) fn parse_contract(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected contract"));
        return None;
    };
    let start = head.span;
    // Optional `abstract`, then the `contract`/`interface`/`library`
    // keyword itself. The kind token is stored so `kind_label` can
    // report interface and library with the right detail; the name
    // stays the first identifier so accessors keep working.
    let mut prefix: Vec<SyntaxElement> = Vec::new();
    if head.kind == SolidityKind::Abstract.as_u16() {
        prefix.push(token_element(head));
        match cursor.peek_kind() {
            Some(kind)
                if kind == SolidityKind::Contract.as_u16()
                    || kind == SolidityKind::Interface.as_u16()
                    || kind == SolidityKind::Library.as_u16() =>
            {
                let Some(kind_token) = cursor.bump() else {
                    errors.push(ParseError::new(cursor.peek_span(), "expected `contract`"));
                    return None;
                };
                prefix.push(token_element(kind_token));
            }
            _ => {
                errors.push(ParseError::new(cursor.peek_span(), "expected `contract`"));
                return None;
            }
        }
    } else if head.kind == SolidityKind::Contract.as_u16()
        || head.kind == SolidityKind::Interface.as_u16()
        || head.kind == SolidityKind::Library.as_u16()
    {
        prefix.push(token_element(head));
    } else {
        errors.push(ParseError::new(head.span, "expected `contract`"));
        return None;
    }
    let name = match cursor.expect(SolidityKind::Ident, "contract name") {
        Ok(name) => name,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    // Optional `abstract`, then optional `is Base, ...` heritage list.
    // Base constructor args like `A(1)` are skipped; only names are stored.
    prefix.push(token_element(name));
    if cursor.peek_kind() == Some(SolidityKind::Abstract.as_u16()) {
        let Some(abstract_token) = cursor.bump() else {
            errors.push(ParseError::new(cursor.peek_span(), "expected `abstract`"));
            return None;
        };
        prefix.push(token_element(abstract_token));
    }
    if cursor.peek_kind() == Some(SolidityKind::Is.as_u16()) {
        cursor.bump();
        loop {
            if cursor.peek_kind() == Some(SolidityKind::Ident.as_u16()) {
                let Some(base) = cursor.bump() else {
                    errors.push(ParseError::new(cursor.peek_span(), "expected base name"));
                    return None;
                };
                prefix.push(token_element(base));
                if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
                    skip_parens(cursor);
                }
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    cursor.bump();
                } else {
                    break;
                }
            } else {
                errors.push(ParseError::new(cursor.peek_span(), "expected base name"));
                // Drop the broken header: skip to the body and over it so
                // one bad heritage stays one error without cascading.
                while let Some(kind) = cursor.peek_kind() {
                    if kind == SolidityKind::LBrace.as_u16() {
                        skip_balanced(cursor);
                        break;
                    }
                    if kind == SolidityKind::Pragma.as_u16()
                        || kind == SolidityKind::Import.as_u16()
                        || kind == SolidityKind::Contract.as_u16()
                        || kind == SolidityKind::Abstract.as_u16()
                        || kind == SolidityKind::Interface.as_u16()
                        || kind == SolidityKind::Library.as_u16()
                        || kind == SolidityKind::Struct.as_u16()
                        || kind == SolidityKind::Enum.as_u16()
                        || kind == SolidityKind::ErrorKw.as_u16()
                        || kind == SolidityKind::Event.as_u16()
                        || kind == SolidityKind::Type.as_u16()
                    {
                        break;
                    }
                    cursor.bump();
                }
                return None;
            }
        }
    }
    if let Err(error) = cursor.expect(SolidityKind::LBrace, "`{`") {
        errors.push(error);
        return None;
    }
    let mut members: Vec<SyntaxElement> = prefix;
    loop {
        match cursor.peek_kind() {
            None => {
                // Typing state: body never closed. Keep the members
                // parsed so far instead of dropping the contract.
                errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
                return Some(SyntaxNode::with_children(
                    SolidityNode::Contract.as_u16(),
                    Span::new(start.start, cursor.peek_span().end),
                    members,
                ));
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => break,
            Some(kind)
                if kind == SolidityKind::Function.as_u16()
                    || kind == SolidityKind::Constructor.as_u16()
                    || kind == SolidityKind::Fallback.as_u16()
                    || kind == SolidityKind::Receive.as_u16() =>
            {
                match parse_function(cursor, errors) {
                    Some(node) => members.push(SyntaxElement::Node(node)),
                    None => recover_member(cursor),
                }
            }
            Some(kind) if kind == SolidityKind::Modifier.as_u16() => {
                match parse_modifier(cursor, errors) {
                    Some(node) => members.push(SyntaxElement::Node(node)),
                    None => recover_member(cursor),
                }
            }
            Some(kind) if kind == SolidityKind::Event.as_u16() => {
                match parse_event(cursor, errors) {
                    Some(node) => members.push(SyntaxElement::Node(node)),
                    None => recover_member(cursor),
                }
            }
            Some(kind) if kind == SolidityKind::Struct.as_u16() => {
                match parse_struct(cursor, errors) {
                    Some(node) => members.push(SyntaxElement::Node(node)),
                    None => recover_member(cursor),
                }
            }
            Some(kind) if kind == SolidityKind::Enum.as_u16() => match parse_enum(cursor, errors) {
                Some(node) => members.push(SyntaxElement::Node(node)),
                None => recover_member(cursor),
            },
            Some(kind) if kind == SolidityKind::ErrorKw.as_u16() => {
                match parse_error(cursor, errors) {
                    Some(node) => members.push(SyntaxElement::Node(node)),
                    None => recover_member(cursor),
                }
            }
            Some(kind) if kind == SolidityKind::Type.as_u16() => {
                match parse_user_type(cursor, errors) {
                    Some(node) => members.push(SyntaxElement::Node(node)),
                    None => recover_member(cursor),
                }
            }
            Some(kind) if is_type_start(kind) || kind == SolidityKind::Ident.as_u16() => {
                match parse_state_var(cursor) {
                    Ok(node) => members.push(SyntaxElement::Node(node)),
                    Err(error) => {
                        errors.push(error);
                        recover_member(cursor);
                    }
                }
            }
            Some(_) => {
                errors.push(ParseError::new(
                    cursor.peek_span(),
                    "expected function or state variable",
                ));
                recover_member(cursor);
            }
        }
    }
    let Some(close) = cursor.bump() else {
        // Unreachable in practice: RBrace was peeked above. Keep the
        // partial node on the defensive path instead of dropping it.
        errors.push(ParseError::new(cursor.peek_span(), "expected `}`"));
        return Some(SyntaxNode::with_children(
            SolidityNode::Contract.as_u16(),
            Span::new(start.start, cursor.peek_span().end),
            members,
        ));
    };
    Some(SyntaxNode::with_children(
        SolidityNode::Contract.as_u16(),
        Span::new(start.start, close.span.end),
        members,
    ))
}

/// Skip to the next member, brace aware: a `}` only stops the search at
/// depth zero. Always moves forward.
pub(crate) fn recover_member(cursor: &mut Cursor) {
    let mut depth = 0u32;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LBrace.as_u16() {
            depth += 1;
        } else if kind == SolidityKind::RBrace.as_u16() {
            if depth == 0 {
                return;
            }
            depth -= 1;
        } else if depth == 0 {
            if kind == SolidityKind::Semicolon.as_u16() {
                cursor.bump();
                return;
            }
            if kind == SolidityKind::Function.as_u16()
                || kind == SolidityKind::Constructor.as_u16()
                || kind == SolidityKind::Fallback.as_u16()
                || kind == SolidityKind::Receive.as_u16()
                || kind == SolidityKind::Modifier.as_u16()
                || kind == SolidityKind::Event.as_u16()
                || kind == SolidityKind::Struct.as_u16()
                || kind == SolidityKind::Enum.as_u16()
                || kind == SolidityKind::ErrorKw.as_u16()
                || kind == SolidityKind::Type.as_u16()
                || is_type_start(kind)
                || kind == SolidityKind::Ident.as_u16()
            {
                return;
            }
        }
        cursor.bump();
    }
}
