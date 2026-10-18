use super::cursor::{Cursor, token_element};
use super::members::parse_param;
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode, Token};

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

/// Parse `Type`, `Type[]`, `Type[N]`, `mapping(K => V)`, and
/// `function (params) internal|external [mutability] [returns (params)]`
/// with array suffixes handled by the caller.
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
        if let Some(name) = take_mapping_key_name(cursor) {
            parts.push(token_element(name));
        }
        let arrow = cursor.expect(SolidityKind::FatArrow, "`=>`")?;
        parts.push(token_element(arrow));
        let value = parse_type_value(cursor)?;
        parts.push(SyntaxElement::Node(value));
        let close = cursor.expect(SolidityKind::RParen, "`)`")?;
        parts.push(token_element(close));
    } else if cursor.peek_kind() == Some(SolidityKind::Function.as_u16()) {
        let func_type = parse_function_type(cursor)?;
        parts.push(SyntaxElement::Node(func_type));
    } else if let Some(kind) = cursor.peek_kind()
        && is_type_start(kind)
    {
        let first = cursor.bump_or_error("type token")?;
        let is_address = first.kind == SolidityKind::Address.as_u16();
        parts.push(token_element(first));
        if is_address && cursor.peek_kind() == Some(SolidityKind::Payable.as_u16()) {
            let payable = cursor.bump_or_error("payable")?;
            parts.push(token_element(payable));
        }
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
    if cursor.peek_kind() == Some(SolidityKind::Function.as_u16()) {
        return Err(ParseError::new(
            cursor.peek_span(),
            "function type not allowed as mapping key",
        ));
    }
    parse_type_value(cursor)
}

pub(crate) fn parse_type_value(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    match cursor.peek_kind() {
        Some(kind)
            if is_type_start(kind)
                || kind == SolidityKind::Ident.as_u16()
                || kind == SolidityKind::Function.as_u16() =>
        {
            parse_type(cursor)
        }
        _ => Err(ParseError::new(cursor.peek_span(), "expected type")),
    }
}

/// Parse `function (params) internal|external [mutability] [returns (params)]`.
///
/// Visibility is required: only `internal` or `external` pass, anything
/// else errors. Mutability (`pure`, `view`, `payable`, `constant`) is
/// optional. Returns is optional with its own param list. Array suffixes
/// stay with the caller so `function (uint) external[]` keeps one node.
pub(crate) fn parse_function_type(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start_span = cursor.peek_span();
    let func = cursor.bump_or_error("function keyword")?;
    if func.kind != SolidityKind::Function.as_u16() {
        return Err(ParseError::new(func.span, "expected type"));
    }
    let mut parts = vec![token_element(func)];
    let open = cursor.expect(SolidityKind::LParen, "`(`")?;
    parts.push(token_element(open));
    if cursor.peek_kind() != Some(SolidityKind::RParen.as_u16()) {
        loop {
            let param = parse_param(cursor)?;
            parts.push(SyntaxElement::Node(param));
            if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                let Some(comma) = cursor.bump() else {
                    return Err(ParseError::new(cursor.peek_span(), "expected `,`"));
                };
                parts.push(token_element(comma));
            } else {
                break;
            }
        }
    }
    let close = cursor.expect(SolidityKind::RParen, "`)`")?;
    parts.push(token_element(close));
    match cursor.peek_kind() {
        Some(kind)
            if kind == SolidityKind::Internal.as_u16()
                || kind == SolidityKind::External.as_u16() =>
        {
            let Some(vis) = cursor.bump() else {
                return Err(ParseError::new(
                    cursor.peek_span(),
                    "expected 'internal' or 'external'",
                ));
            };
            parts.push(token_element(vis));
        }
        _ => {
            return Err(ParseError::new(
                cursor.peek_span(),
                "expected 'internal' or 'external'",
            ));
        }
    }
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::Pure.as_u16()
            || kind == SolidityKind::View.as_u16()
            || kind == SolidityKind::Payable.as_u16()
            || kind == SolidityKind::Constant.as_u16()
        {
            let Some(muta) = cursor.bump() else {
                break;
            };
            parts.push(token_element(muta));
        } else {
            break;
        }
    }
    if cursor.peek_kind() == Some(SolidityKind::Returns.as_u16()) {
        let Some(ret) = cursor.bump() else {
            return Err(ParseError::new(cursor.peek_span(), "expected returns"));
        };
        parts.push(token_element(ret));
        let open_ret = cursor.expect(SolidityKind::LParen, "`(`")?;
        parts.push(token_element(open_ret));
        if cursor.peek_kind() != Some(SolidityKind::RParen.as_u16()) {
            loop {
                let param = parse_param(cursor)?;
                parts.push(SyntaxElement::Node(param));
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    let Some(comma) = cursor.bump() else {
                        return Err(ParseError::new(cursor.peek_span(), "expected `,`"));
                    };
                    parts.push(token_element(comma));
                } else {
                    break;
                }
            }
        }
        let close_ret = cursor.expect(SolidityKind::RParen, "`)`")?;
        parts.push(token_element(close_ret));
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

/// True when `pos` starts a function type: `function` followed by `(`.
///
/// Skips comments for the lookahead. Definitions (`function foo`) return
/// false since the next code token is an ident, not a paren.
pub(crate) fn is_function_type_at(tokens: &[Token], pos: usize) -> bool {
    let comment = SolidityKind::Comment.as_u16();
    let Some(first) = tokens.get(pos) else {
        return false;
    };
    if first.kind != SolidityKind::Function.as_u16() {
        return false;
    }
    let mut index = pos + 1;
    while tokens.get(index).is_some_and(|token| token.kind == comment) {
        index += 1;
    }
    tokens
        .get(index)
        .is_some_and(|token| token.kind == SolidityKind::LParen.as_u16())
}

/// End index past a function type at `pos`, or `None` when absent.
///
/// Covers `function (...) vis [mutability] [returns (...)]` plus array
/// suffixes. Lenient on visibility so invalid kinds still count as a
/// function type and surface the precise visibility error later.
pub(crate) fn skip_function_type_at(tokens: &[Token], pos: usize) -> Option<usize> {
    if !is_function_type_at(tokens, pos) {
        return None;
    }
    let comment = SolidityKind::Comment.as_u16();
    let mut index = pos + 1;
    while tokens.get(index).is_some_and(|token| token.kind == comment) {
        index += 1;
    }
    let open = tokens.get(index)?;
    if open.kind != SolidityKind::LParen.as_u16() {
        return None;
    }
    let mut depth = 0u32;
    let mut cursor_idx = index;
    let mut close_idx = None;
    while let Some(token) = tokens.get(cursor_idx) {
        if token.kind == comment {
            cursor_idx += 1;
            continue;
        }
        if token.kind == SolidityKind::LParen.as_u16() {
            depth += 1;
        } else if token.kind == SolidityKind::RParen.as_u16() {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                close_idx = Some(cursor_idx);
                break;
            }
        } else if token.kind == SolidityKind::LBrace.as_u16()
            || token.kind == SolidityKind::RBrace.as_u16()
        {
            return None;
        }
        cursor_idx += 1;
    }
    let close = close_idx?;
    index = close + 1;
    while tokens.get(index).is_some_and(|token| token.kind == comment) {
        index += 1;
    }
    let vis = tokens.get(index)?;
    if vis.kind == SolidityKind::Internal.as_u16()
        || vis.kind == SolidityKind::External.as_u16()
        || vis.kind == SolidityKind::Public.as_u16()
        || vis.kind == SolidityKind::Private.as_u16()
    {
        index += 1;
        while tokens.get(index).is_some_and(|token| token.kind == comment) {
            index += 1;
        }
    }
    while let Some(next) = tokens.get(index) {
        if next.kind == comment {
            index += 1;
            continue;
        }
        if next.kind == SolidityKind::Pure.as_u16()
            || next.kind == SolidityKind::View.as_u16()
            || next.kind == SolidityKind::Payable.as_u16()
            || next.kind == SolidityKind::Constant.as_u16()
        {
            index += 1;
            while tokens.get(index).is_some_and(|token| token.kind == comment) {
                index += 1;
            }
        } else {
            break;
        }
    }
    if tokens
        .get(index)
        .is_some_and(|token| token.kind == SolidityKind::Returns.as_u16())
    {
        index += 1;
        while tokens.get(index).is_some_and(|token| token.kind == comment) {
            index += 1;
        }
        if !tokens
            .get(index)
            .is_some_and(|token| token.kind == SolidityKind::LParen.as_u16())
        {
            return None;
        }
        let mut ret_depth = 0u32;
        let mut ret_cursor = index;
        let mut ret_close = None;
        while let Some(token) = tokens.get(ret_cursor) {
            if token.kind == comment {
                ret_cursor += 1;
                continue;
            }
            if token.kind == SolidityKind::LParen.as_u16() {
                ret_depth += 1;
            } else if token.kind == SolidityKind::RParen.as_u16() {
                if ret_depth == 0 {
                    return None;
                }
                ret_depth -= 1;
                if ret_depth == 0 {
                    ret_close = Some(ret_cursor);
                    break;
                }
            } else if token.kind == SolidityKind::LBrace.as_u16()
                || token.kind == SolidityKind::RBrace.as_u16()
            {
                return None;
            }
            ret_cursor += 1;
        }
        index = ret_close? + 1;
        while tokens.get(index).is_some_and(|token| token.kind == comment) {
            index += 1;
        }
    }
    loop {
        while tokens.get(index).is_some_and(|token| token.kind == comment) {
            index += 1;
        }
        if !tokens
            .get(index)
            .is_some_and(|token| token.kind == SolidityKind::LBracket.as_u16())
        {
            break;
        }
        index += 1;
        loop {
            while tokens.get(index).is_some_and(|token| token.kind == comment) {
                index += 1;
            }
            let inner = tokens.get(index)?;
            if inner.kind == SolidityKind::RBracket.as_u16() {
                index += 1;
                break;
            }
            if inner.kind == SolidityKind::RBrace.as_u16()
                || inner.kind == SolidityKind::LBrace.as_u16()
                || inner.kind == SolidityKind::Semicolon.as_u16()
            {
                return None;
            }
            index += 1;
        }
    }
    Some(index)
}

fn is_name_word(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
}

/// Take `mapping(Key name => Value)` key name when present.
///
/// Peeks the current word plus the token after it: only when the next
/// code token is `=>` is the word a name, so `mapping(address => uint)`
/// keeps no name while `mapping(address account => uint)` keeps one.
fn take_mapping_key_name(cursor: &mut Cursor) -> Option<Token> {
    let current = cursor.peek()?.clone();
    if !is_name_word(&current.text) {
        return None;
    }
    let comment = SolidityKind::Comment.as_u16();
    let tokens = cursor.tokens;
    let mut index = cursor.pos() + 1;
    while let Some(token) = tokens.get(index) {
        if token.kind == comment {
            index += 1;
            continue;
        }
        if token.kind == SolidityKind::FatArrow.as_u16() {
            return cursor.bump();
        }
        return None;
    }
    None
}
