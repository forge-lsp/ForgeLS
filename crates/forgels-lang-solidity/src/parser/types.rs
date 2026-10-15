use super::cursor::{Cursor, token_element};
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode};

/// True when a token kind can start a type: elementary, sized, mapping,
/// or a user type name.
pub(crate) fn is_type_start(kind: u16) -> bool {
    const TYPES: &[SolidityKind] = &[
        SolidityKind::Bool,
        SolidityKind::Address,
        SolidityKind::String,
        SolidityKind::Bytes,
        SolidityKind::Mapping,
        SolidityKind::Uint,
        SolidityKind::Int,
        SolidityKind::UintSized,
        SolidityKind::IntSized,
        SolidityKind::BytesSized,
        SolidityKind::Fixed,
        SolidityKind::Ufixed,
    ];
    TYPES.iter().any(|type_kind| type_kind.as_u16() == kind)
}

/// Parse `Type`, `Type[]`, `Type[N]`, and `mapping(K => V)` with suffixes.
pub(crate) fn parse_type(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start_span = cursor.peek_span();
    let mut parts: Vec<SyntaxElement> = Vec::new();
    if cursor.peek_kind() == Some(SolidityKind::Mapping.as_u16()) {
        let mapping = cursor.bump_or_error("mapping keyword")?;
        parts.push(token_element(mapping));
        let open = cursor.expect(SolidityKind::LParen, "`(`")?;
        parts.push(token_element(open));
        let key = parse_mapping_key(cursor)?;
        parts.push(SyntaxElement::Node(key));
        let arrow = cursor.expect(SolidityKind::FatArrow, "`=>`")?;
        parts.push(token_element(arrow));
        let value = parse_type_value(cursor)?;
        parts.push(SyntaxElement::Node(value));
        let close = cursor.expect(SolidityKind::RParen, "`)`")?;
        parts.push(token_element(close));
    } else if let Some(kind) = cursor.peek_kind()
        && is_type_start(kind)
    {
        let first = cursor.bump_or_error("type token")?;
        parts.push(token_element(first));
    } else if cursor.peek_kind() == Some(SolidityKind::Ident.as_u16()) {
        // User type outside the beta subset: keep the name, flag nothing.
        let name = cursor.bump_or_error("type name")?;
        parts.push(token_element(name));
    } else {
        return Err(ParseError::new(cursor.peek_span(), "expected type"));
    }
    while cursor.peek_kind() == Some(SolidityKind::LBracket.as_u16()) {
        let open = cursor.bump_or_error("bracket")?;
        parts.push(token_element(open));
        if cursor.peek_kind() == Some(SolidityKind::Number.as_u16()) {
            let size = cursor.bump_or_error("array size")?;
            parts.push(token_element(size));
        }
        let close = cursor.expect(SolidityKind::RBracket, "`]`")?;
        parts.push(token_element(close));
    }
    let end = parts
        .last()
        .map(|element| match element {
            SyntaxElement::Token(token) => token.span.end,
            SyntaxElement::Node(node) => node.span.end,
        })
        .unwrap_or(start_span.end);
    Ok(SyntaxNode::with_children(
        SolidityNode::Type.as_u16(),
        Span::new(start_span.start, end),
        parts,
    ))
}

pub(crate) fn parse_mapping_key(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    parse_type_value(cursor)
}

pub(crate) fn parse_type_value(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    match cursor.peek_kind() {
        Some(kind) if is_type_start(kind) || kind == SolidityKind::Ident.as_u16() => {
            parse_type(cursor)
        }
        _ => Err(ParseError::new(cursor.peek_span(), "expected type")),
    }
}
