use super::cursor::{Cursor, token_element};
use super::types::{is_type_start, parse_type};
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode};

/// Parse `Type name [= init] ;` at contract level.
pub(crate) fn parse_state_var(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    parse_var(cursor, SolidityNode::StateVar)
}

/// Parse `Type name [= init] ;` inside a block.
pub(crate) fn parse_local_var(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    parse_var(cursor, SolidityNode::LocalVar)
}

pub(crate) fn parse_var(cursor: &mut Cursor, kind: SolidityNode) -> Result<SyntaxNode, ParseError> {
    let var_type = parse_type(cursor)?;
    let type_end = var_type.span.end;
    // Data location sits between type and name: `string memory label`.
    if matches!(
        cursor.peek_kind(),
        Some(kind)
            if kind == SolidityKind::Memory.as_u16()
                || kind == SolidityKind::Storage.as_u16()
                || kind == SolidityKind::Calldata.as_u16()
    ) {
        cursor.bump();
    }
    let name = cursor.expect(SolidityKind::Ident, "variable name")?;
    let mut end = name.span.end.max(type_end);
    if cursor.peek_kind() == Some(SolidityKind::Assign.as_u16()) {
        cursor.bump();
        // Initializer expressions belong to a later leaf. Skip one level.
        while let Some(kind) = cursor.peek_kind() {
            if kind == SolidityKind::Semicolon.as_u16() || kind == SolidityKind::RBrace.as_u16() {
                break;
            }
            if kind == SolidityKind::LBrace.as_u16() {
                skip_balanced(cursor);
            } else {
                cursor.bump();
            }
        }
        end = cursor
            .tokens
            .get(cursor.pos().saturating_sub(1))
            .map(|token| token.span.end)
            .unwrap_or(end);
    }
    let semi = cursor.expect(SolidityKind::Semicolon, "`;`")?;
    end = end.max(semi.span.end);
    Ok(SyntaxNode::with_children(
        kind.as_u16(),
        Span::new(var_type.span.start, end),
        vec![SyntaxElement::Node(var_type), token_element(name)],
    ))
}

/// Parse function name(params) modifiers returns (types) body.
/// Constructor follows the same shape with an implied name.
/// Structural failures push one error and return None. Body local
/// errors go to errors while valid locals stay in the block.
pub(crate) fn parse_function(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(head) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected function"));
        return None;
    };
    let start = head.span;
    let name = if head.kind == SolidityKind::Constructor.as_u16() {
        head.clone()
    } else {
        match cursor.expect(SolidityKind::Ident, "function name") {
            Ok(name) => name,
            Err(error) => {
                errors.push(error);
                return None;
            }
        }
    };
    let mut children = vec![token_element(name)];
    if cursor.expect(SolidityKind::LParen, "`(`").is_err() {
        errors.push(ParseError::new(cursor.peek_span(), "expected `(`"));
        return None;
    }
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
    // Modifiers and visibility are consumed, not stored.
    loop {
        match cursor.peek_kind() {
            Some(kind)
                if kind == SolidityKind::Public.as_u16()
                    || kind == SolidityKind::Private.as_u16()
                    || kind == SolidityKind::Internal.as_u16()
                    || kind == SolidityKind::External.as_u16()
                    || kind == SolidityKind::Pure.as_u16()
                    || kind == SolidityKind::View.as_u16()
                    || kind == SolidityKind::Payable.as_u16()
                    || kind == SolidityKind::Constant.as_u16()
                    || kind == SolidityKind::Virtual.as_u16()
                    || kind == SolidityKind::Override.as_u16() =>
            {
                cursor.bump();
            }
            Some(kind) if kind == SolidityKind::Returns.as_u16() => {
                cursor.bump();
                if cursor.expect(SolidityKind::LParen, "`(`").is_err() {
                    errors.push(ParseError::new(cursor.peek_span(), "expected `(`"));
                    return None;
                }
                let returns_start = cursor
                    .tokens
                    .get(cursor.pos().saturating_sub(1))
                    .map(|token| token.span.start)
                    .unwrap_or(start.start);
                let mut returns_children = Vec::new();
                if cursor.peek_kind() != Some(SolidityKind::RParen.as_u16()) {
                    loop {
                        match parse_param(cursor) {
                            Ok(param) => returns_children.push(SyntaxElement::Node(param)),
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
                match cursor.expect(SolidityKind::RParen, "`)`") {
                    Ok(close) => {
                        children.push(SyntaxElement::Node(SyntaxNode::with_children(
                            SolidityNode::Returns.as_u16(),
                            Span::new(returns_start, close.span.end),
                            returns_children,
                        )));
                    }
                    Err(_) => {
                        errors.push(ParseError::new(cursor.peek_span(), "expected `)`"));
                        return None;
                    }
                }
            }
            _ => break,
        }
    }
    match parse_block(cursor, errors) {
        Some(body) => {
            let end = body.span.end;
            children.push(SyntaxElement::Node(body));
            Some(SyntaxNode::with_children(
                SolidityNode::Function.as_u16(),
                Span::new(start.start, end),
                children,
            ))
        }
        None => None,
    }
}

/// Parse `Type name?` inside parameter lists.
pub(crate) fn parse_param(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let param_type = parse_type(cursor)?;
    let start = param_type.span.start;
    let mut children = vec![SyntaxElement::Node(param_type)];
    // Data location sits between type and name: `string memory label`.
    if matches!(
        cursor.peek_kind(),
        Some(kind)
            if kind == SolidityKind::Memory.as_u16()
                || kind == SolidityKind::Storage.as_u16()
                || kind == SolidityKind::Calldata.as_u16()
    ) {
        cursor.bump();
    }
    let mut end = children
        .first()
        .and_then(|element| match element {
            SyntaxElement::Node(node) => Some(node.span.end),
            _ => None,
        })
        .unwrap_or(start);
    if cursor.peek_kind() == Some(SolidityKind::Ident.as_u16()) {
        let name = cursor.bump_or_error("param name")?;
        end = name.span.end;
        children.push(token_element(name));
    }
    Ok(SyntaxNode::with_children(
        SolidityNode::Param.as_u16(),
        Span::new(start, end),
        children,
    ))
}

/// Parse `{ ... }`, keeping local variable declarations and the full span.
/// Local errors go to `errors` while valid locals stay. EOF yields the
/// partial block so typing inside a body keeps working. `None` means no
/// `{` ever opened.
pub(crate) fn parse_block(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let open = match cursor.expect(SolidityKind::LBrace, "`{`") {
        Ok(open) => open,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    let mut locals: Vec<SyntaxElement> = Vec::new();
    loop {
        match cursor.peek_kind() {
            None => {
                errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
                return Some(SyntaxNode::with_children(
                    SolidityNode::Block.as_u16(),
                    Span::new(open.span.start, cursor.peek_span().end),
                    locals,
                ));
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => break,
            Some(kind) if kind == SolidityKind::Assembly.as_u16() => {
                skip_assembly(cursor);
            }
            Some(kind)
                if (is_type_start(kind) || kind == SolidityKind::Ident.as_u16())
                    && is_local_decl_ahead(cursor) =>
            {
                match parse_local_var(cursor) {
                    Ok(node) => locals.push(SyntaxElement::Node(node)),
                    Err(error) => {
                        errors.push(error);
                        recover_statement(cursor);
                    }
                }
            }
            Some(kind) if is_type_start(kind) || kind == SolidityKind::Ident.as_u16() => {
                if kind == SolidityKind::Ident.as_u16()
                    && let Some(call) = try_parse_call(cursor)
                {
                    locals.push(SyntaxElement::Node(call));
                } else {
                    // Type-like token without a following name: an expression
                    // statement, skipped like any other.
                    recover_statement(cursor);
                }
            }
            _ => {
                recover_statement(cursor);
            }
        }
    }
    let Some(close) = cursor.bump() else {
        // Unreachable in practice: RBrace was peeked above. Keep the
        // partial block on the defensive path instead of dropping it.
        errors.push(ParseError::new(cursor.peek_span(), "expected `}`"));
        return Some(SyntaxNode::with_children(
            SolidityNode::Block.as_u16(),
            Span::new(open.span.start, cursor.peek_span().end),
            locals,
        ));
    };
    Some(SyntaxNode::with_children(
        SolidityNode::Block.as_u16(),
        Span::new(open.span.start, close.span.end),
        locals,
    ))
}

/// Try `name(args)` at block top level. Returns the Call node on success.
///
/// Plain calls only: member access (`a.b()`), named args (`f({x: 1})`),
/// and `new`/`super`/`constructor` fall back to `recover_statement`
/// with no node and no error. Commas count at paren depth one with
/// no open bracket; empty parens give zero args.
pub(crate) fn try_parse_call(cursor: &mut Cursor) -> Option<SyntaxNode> {
    let tokens = cursor.tokens;
    let comment = SolidityKind::Comment.as_u16();
    let next_code = |mut index: usize| -> Option<usize> {
        while let Some(token) = tokens.get(index) {
            if token.kind != comment {
                return Some(index);
            }
            index += 1;
        }
        None
    };
    let callee_idx = next_code(cursor.pos())?;
    let callee = tokens.get(callee_idx)?.clone();
    if callee.kind != SolidityKind::Ident.as_u16() {
        return None;
    }
    if callee.text == "super" || callee.text == "constructor" || callee.text == "new" {
        return None;
    }
    let lparen_idx = next_code(callee_idx + 1)?;
    if tokens.get(lparen_idx)?.kind != SolidityKind::LParen.as_u16() {
        return None;
    }
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut commas = Vec::new();
    let mut rparen_idx = None;
    let mut index = lparen_idx;
    while let Some(token) = tokens.get(index) {
        if token.kind == comment {
            index += 1;
            continue;
        }
        let kind = token.kind;
        if kind == SolidityKind::LBrace.as_u16() {
            return None;
        }
        if kind == SolidityKind::LParen.as_u16() {
            paren_depth += 1;
        } else if kind == SolidityKind::RParen.as_u16() {
            if paren_depth == 0 {
                return None;
            }
            paren_depth -= 1;
            if paren_depth == 0 {
                rparen_idx = Some(index);
                break;
            }
        } else if kind == SolidityKind::LBracket.as_u16() {
            bracket_depth += 1;
        } else if kind == SolidityKind::RBracket.as_u16() {
            if bracket_depth == 0 {
                return None;
            }
            bracket_depth -= 1;
        } else if kind == SolidityKind::Comma.as_u16() && paren_depth == 1 && bracket_depth == 0 {
            commas.push(token.clone());
        }
        index += 1;
    }
    let rparen_idx = rparen_idx?;
    let rparen = tokens.get(rparen_idx)?.clone();
    let after_lparen = next_code(lparen_idx + 1)?;
    let is_empty = after_lparen == rparen_idx;
    let mut children = vec![token_element(callee.clone())];
    if !is_empty {
        children.push(token_element(tokens.get(lparen_idx)?.clone()));
        for comma in commas {
            children.push(token_element(comma));
        }
    }
    let node = SyntaxNode::with_children(
        SolidityNode::Call.as_u16(),
        Span::new(callee.span.start, rparen.span.end),
        children,
    );
    while cursor.pos() <= rparen_idx {
        if cursor.bump().is_none() {
            break;
        }
    }
    Some(node)
}

/// Skip an inline assembly block without errors.
///
/// Full Yul parsing is deferred: the optional dialect string and the
/// balanced brace body are consumed so surrounding locals keep parsing.
pub(crate) fn skip_assembly(cursor: &mut Cursor) {
    cursor.bump();
    if cursor.peek_kind() == Some(SolidityKind::StringLit.as_u16()) {
        cursor.bump();
    }
    if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
        skip_parens(cursor);
    }
    if cursor.peek_kind() == Some(SolidityKind::LBrace.as_u16()) {
        skip_balanced(cursor);
    }
}

/// True when the upcoming tokens look like `Type name [= ;]`.
pub(crate) fn is_local_decl_ahead(cursor: &mut Cursor) -> bool {
    let tokens = &cursor.tokens[cursor.pos()..];
    let mut index = 0;
    // Skip the type: mapping(...) or one word, plus array suffixes.
    if tokens.first().is_some_and(|token| {
        token.kind == SolidityKind::Mapping.as_u16()
            && tokens
                .get(1)
                .is_some_and(|next| next.kind == SolidityKind::LParen.as_u16())
    }) {
        let mut depth = 0;
        while let Some(token) = tokens.get(index) {
            if token.kind == SolidityKind::LParen.as_u16() {
                depth += 1;
            } else if token.kind == SolidityKind::RParen.as_u16() {
                depth -= 1;
                if depth == 0 {
                    index += 1;
                    break;
                }
            }
            index += 1;
        }
    } else {
        index = 1;
    }
    // Step over array suffixes `[N]` and `[]` in the lookahead.
    while tokens
        .get(index)
        .is_some_and(|token| token.kind == SolidityKind::LBracket.as_u16())
    {
        index += 1;
        while let Some(token) = tokens.get(index) {
            index += 1;
            if token.kind == SolidityKind::RBracket.as_u16() {
                break;
            }
        }
    }
    // Data location sits between type and name: skip one if present.
    if tokens.get(index).is_some_and(|token| {
        token.kind == SolidityKind::Memory.as_u16()
            || token.kind == SolidityKind::Storage.as_u16()
            || token.kind == SolidityKind::Calldata.as_u16()
    }) {
        index += 1;
    }
    matches!(
        tokens.get(index).map(|token| token.kind),
        Some(kind) if kind == SolidityKind::Ident.as_u16()
    )
}

/// Skip one statement: to `;`, over a balanced block, or stop at `}`.
pub(crate) fn recover_statement(cursor: &mut Cursor) {
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::Semicolon.as_u16() {
            cursor.bump();
            return;
        }
        if kind == SolidityKind::RBrace.as_u16() {
            return;
        }
        if kind == SolidityKind::LBrace.as_u16() {
            skip_balanced(cursor);
            return;
        }
        cursor.bump();
    }
}

/// Skip a balanced `( ... )` starting at the opening paren.
pub(crate) fn skip_parens(cursor: &mut Cursor) {
    let mut depth = 0u32;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LParen.as_u16() {
            depth += 1;
        } else if kind == SolidityKind::RParen.as_u16() {
            depth -= 1;
            cursor.bump();
            if depth == 0 {
                return;
            }
            continue;
        }
        cursor.bump();
    }
}

/// Skip a balanced `{ ... }` starting at the opening brace.
pub(crate) fn skip_balanced(cursor: &mut Cursor) {
    let mut depth = 0u32;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LBrace.as_u16() {
            depth += 1;
        } else if kind == SolidityKind::RBrace.as_u16() {
            depth -= 1;
            cursor.bump();
            if depth == 0 {
                return;
            }
            continue;
        }
        cursor.bump();
    }
}
