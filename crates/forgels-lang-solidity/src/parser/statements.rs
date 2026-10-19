use super::cursor::{Cursor, token_element};
use super::members::recover_statement;
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode};

/// Parse `return [expr] ;` with verbatim tokens through `;`.
///
/// Missing `;` pushes one error and recovers with no node.
pub(crate) fn parse_return(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(keyword) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected return"));
        return None;
    };
    let start = keyword.span.start;
    let mut children = vec![token_element(keyword)];
    match collect_to_semi(cursor, &mut children) {
        Some(end) => Some(SyntaxNode::with_children(
            SolidityNode::Return.as_u16(),
            Span::new(start, end),
            children,
        )),
        None => {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            recover_statement(cursor);
            None
        }
    }
}

/// Parse `break ;`. Anything but `;` errors and recovers with no node.
pub(crate) fn parse_break(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let Some(keyword) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected break"));
        return None;
    };
    let start = keyword.span.start;
    match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => Some(SyntaxNode::with_children(
            SolidityNode::Break.as_u16(),
            Span::new(start, semi.span.end),
            vec![token_element(keyword), token_element(semi)],
        )),
        Err(_) => {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            recover_statement(cursor);
            None
        }
    }
}

/// Parse `continue ;`. Anything but `;` errors and recovers with no node.
pub(crate) fn parse_continue(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(keyword) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected continue"));
        return None;
    };
    let start = keyword.span.start;
    match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => Some(SyntaxNode::with_children(
            SolidityNode::Continue.as_u16(),
            Span::new(start, semi.span.end),
            vec![token_element(keyword), token_element(semi)],
        )),
        Err(_) => {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            recover_statement(cursor);
            None
        }
    }
}

/// Parse `delete <expr> ;` with verbatim tokens through `;`.
///
/// Bare `delete ;` stays lenient with keyword plus semi. Missing `;`
/// pushes one error and recovers with no node.
pub(crate) fn parse_delete(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(keyword) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected delete"));
        return None;
    };
    let start = keyword.span.start;
    let mut children = vec![token_element(keyword)];
    if cursor.peek_kind() == Some(SolidityKind::Semicolon.as_u16()) {
        let Some(semi) = cursor.bump() else {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            return None;
        };
        children.push(token_element(semi.clone()));
        return Some(SyntaxNode::with_children(
            SolidityNode::Delete.as_u16(),
            Span::new(start, semi.span.end),
            children,
        ));
    }
    match collect_to_semi(cursor, &mut children) {
        Some(end) => Some(SyntaxNode::with_children(
            SolidityNode::Delete.as_u16(),
            Span::new(start, end),
            children,
        )),
        None => {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            recover_statement(cursor);
            None
        }
    }
}

/// Parse `do <body> while (cond) ;` with verbatim tokens.
///
/// The body stays verbatim (brace group or single `;` row) with no nested
/// Block child, so `child_blocks` and `all_*` counts stay unchanged.
/// P2 slicing: conditions use balanced skips like `if`/`while`/`for`;
/// only bodies gain nodes. Missing `while` pushes one error and returns
/// with no recover so the next sibling still parses. Later tail failures
/// push one error and recover with no node.
pub(crate) fn parse_do_while(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    let Some(do_token) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected do"));
        return None;
    };
    let start = do_token.span.start;
    let mut children = vec![token_element(do_token)];
    if cursor.peek_kind() == Some(SolidityKind::LBrace.as_u16()) {
        if collect_balanced_braces(cursor, &mut children).is_none() {
            errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
            recover_statement(cursor);
            return None;
        }
    } else if collect_to_semi(cursor, &mut children).is_none() {
        errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
        recover_statement(cursor);
        return None;
    }
    if cursor.peek_kind() != Some(SolidityKind::While.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `while`"));
        return None;
    }
    let Some(while_token) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected `while`"));
        return None;
    };
    children.push(token_element(while_token));
    if cursor.peek_kind() != Some(SolidityKind::LParen.as_u16()) {
        errors.push(ParseError::new(cursor.peek_span(), "expected `(`"));
        recover_statement(cursor);
        return None;
    }
    let Some(open) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected `(`"));
        return None;
    };
    children.push(token_element(open));
    if collect_to_rparen(cursor, &mut children).is_none() {
        errors.push(ParseError::new(cursor.peek_span(), "expected `)`"));
        recover_statement(cursor);
        return None;
    }
    match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => {
            let end = semi.span.end;
            children.push(token_element(semi));
            Some(SyntaxNode::with_children(
                SolidityNode::DoWhile.as_u16(),
                Span::new(start, end),
                children,
            ))
        }
        Err(_) => {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            recover_statement(cursor);
            None
        }
    }
}

/// Parse one bare `<expr> ;` row with verbatim tokens.
///
/// Lone `;` bumps with no node and no error. Missing `;` before `}` or
/// EOF stays silent with no node (typing prefixes like `to` must not
/// error); recovery stops at `}` so siblings still parse. Span runs from
/// the first token through the closing `;`.
pub(crate) fn parse_expr_stmt(
    cursor: &mut Cursor,
    _errors: &mut Vec<ParseError>,
) -> Option<SyntaxNode> {
    if cursor.peek_kind() == Some(SolidityKind::Semicolon.as_u16()) {
        cursor.bump();
        return None;
    }
    let start = cursor.peek_span().start;
    let mut children: Vec<SyntaxElement> = Vec::new();
    match collect_to_semi(cursor, &mut children) {
        Some(end) => {
            if children.is_empty() {
                return None;
            }
            Some(SyntaxNode::with_children(
                SolidityNode::ExprStmt.as_u16(),
                Span::new(start, end),
                children,
            ))
        }
        None => {
            recover_statement(cursor);
            None
        }
    }
}

/// Collect verbatim tokens through the first `;`.
///
/// Stops at the first `;` with no paren or bracket check, and before
/// outer `}` when brace depth is zero regardless of `(` or `[` depth
/// with `None` and no consumption of the stop token. Returns the
/// closing `;` end offset on success.
fn collect_to_semi(cursor: &mut Cursor, out: &mut Vec<SyntaxElement>) -> Option<u32> {
    let mut brace = 0u32;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::Semicolon.as_u16() {
            let semi = cursor.bump()?;
            let end = semi.span.end;
            out.push(token_element(semi));
            return Some(end);
        }
        if kind == SolidityKind::RBrace.as_u16() && brace == 0 {
            return None;
        }
        if kind == SolidityKind::LBrace.as_u16() {
            brace += 1;
        } else if kind == SolidityKind::RBrace.as_u16() {
            brace = brace.saturating_sub(1);
        }
        let token = cursor.bump()?;
        out.push(token_element(token));
    }
    None
}

/// Collect a balanced `{ ... }` group verbatim, assuming cursor at `{`.
///
/// Pushes every token including braces. Returns `None` on EOF before
/// close with the cursor at EOF.
fn collect_balanced_braces(cursor: &mut Cursor, out: &mut Vec<SyntaxElement>) -> Option<()> {
    let open = cursor.bump()?;
    out.push(token_element(open));
    let mut depth = 1u32;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LBrace.as_u16() {
            depth += 1;
        } else if kind == SolidityKind::RBrace.as_u16() {
            depth -= 1;
            let close = cursor.bump()?;
            out.push(token_element(close));
            if depth == 0 {
                return Some(());
            }
            continue;
        }
        let token = cursor.bump()?;
        out.push(token_element(token));
    }
    None
}

/// Collect through the matching `)` verbatim, assuming `(` consumed.
///
/// Pushes every token including the close paren. Returns `None` on EOF,
/// on outer `}` before close, or on `{` inside (P2 slicing keeps Yul and
/// options out of conditions).
fn collect_to_rparen(cursor: &mut Cursor, out: &mut Vec<SyntaxElement>) -> Option<()> {
    let mut depth = 1u32;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LParen.as_u16() {
            depth += 1;
        } else if kind == SolidityKind::RParen.as_u16() {
            depth -= 1;
            let close = cursor.bump()?;
            out.push(token_element(close));
            if depth == 0 {
                return Some(());
            }
            continue;
        } else if kind == SolidityKind::LBrace.as_u16() || kind == SolidityKind::RBrace.as_u16() {
            return None;
        }
        let token = cursor.bump()?;
        out.push(token_element(token));
    }
    None
}
