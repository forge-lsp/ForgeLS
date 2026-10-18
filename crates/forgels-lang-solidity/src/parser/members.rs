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
    // State modifiers sit between type and name: `uint public constant x`.
    // `public`, `virtual`, and `override` are stored for getters and
    // override checks; the rest stay consumed so `StateVar::name` keeps
    // working. `payable` covers `address payable owner`: part of the type.
    let mut stored_mods: Vec<SyntaxElement> = Vec::new();
    loop {
        match cursor.peek_kind() {
            Some(kind)
                if kind == SolidityKind::Public.as_u16()
                    || kind == SolidityKind::Virtual.as_u16()
                    || kind == SolidityKind::Override.as_u16() =>
            {
                if let Some(token) = cursor.bump() {
                    stored_mods.push(token_element(token));
                    if kind == SolidityKind::Override.as_u16()
                        && cursor.peek_kind() == Some(SolidityKind::LParen.as_u16())
                    {
                        skip_parens(cursor);
                    }
                } else {
                    break;
                }
            }
            Some(kind)
                if kind == SolidityKind::Private.as_u16()
                    || kind == SolidityKind::Internal.as_u16()
                    || kind == SolidityKind::Constant.as_u16()
                    || kind == SolidityKind::Immutable.as_u16()
                    || kind == SolidityKind::Transient.as_u16()
                    || kind == SolidityKind::Payable.as_u16() =>
            {
                cursor.bump();
            }
            _ => break,
        }
    }
    // Names accept any word token, not only Ident: `after` lexes as
    // reserved yet SBE-style code uses it as `uint after`. Structural
    // tokens never match, so missing names still fail cleanly.
    let name = if let Some(token) = cursor.peek().cloned()
        && is_param_name(&token.text)
    {
        cursor.bump_or_error("variable name")?
    } else {
        return Err(ParseError::new(
            cursor.peek_span(),
            "expected variable name",
        ));
    };
    let mut end = name.span.end.max(type_end);
    let mut init_literal = None;
    if cursor.peek_kind() == Some(SolidityKind::Assign.as_u16()) {
        cursor.bump();
        init_literal = single_literal_ahead(cursor);
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
    let type_start = var_type.span.start;
    let mut children = vec![SyntaxElement::Node(var_type)];
    children.extend(stored_mods);
    children.push(token_element(name));
    if let Some(literal) = init_literal {
        children.push(token_element(literal));
    }
    Ok(SyntaxNode::with_children(
        kind.as_u16(),
        Span::new(type_start, end),
        children,
    ))
}

/// One literal token after `=` when the initializer is exactly one literal.
///
/// Skips comments and returns the token only for a single literal kind
/// directly before `;` or `}`. Multi token forms like `1 ether`, idents,
/// calls, casts, and binops yield `None`.
fn single_literal_ahead(cursor: &mut Cursor) -> Option<forgels_syntax::Token> {
    let comment = SolidityKind::Comment.as_u16();
    let mut found = None;
    let mut count = 0u32;
    let mut index = cursor.pos();
    while let Some(token) = cursor.tokens.get(index) {
        if token.kind == comment {
            index += 1;
            continue;
        }
        if token.kind == SolidityKind::Semicolon.as_u16()
            || token.kind == SolidityKind::RBrace.as_u16()
        {
            break;
        }
        if token.kind == SolidityKind::LBrace.as_u16() {
            return None;
        }
        count += 1;
        if count > 1 {
            return None;
        }
        found = Some(token.clone());
        index += 1;
    }
    let literal = found?;
    if count != 1 {
        return None;
    }
    match literal.kind {
        kind if kind == SolidityKind::Number.as_u16()
            || kind == SolidityKind::OctalNumber.as_u16()
            || kind == SolidityKind::StringLit.as_u16()
            || kind == SolidityKind::HexString.as_u16()
            || kind == SolidityKind::UnicodeString.as_u16()
            || kind == SolidityKind::True.as_u16()
            || kind == SolidityKind::False.as_u16() =>
        {
            Some(literal)
        }
        _ => None,
    }
}

/// Parse function name(params) modifiers returns (types) body.
/// Constructor, fallback, and receive follow the same shape with an
/// implied name. Interface stubs end with `;` and keep no body.
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
    let name = if head.kind == SolidityKind::Constructor.as_u16()
        || head.kind == SolidityKind::Fallback.as_u16()
        || head.kind == SolidityKind::Receive.as_u16()
    {
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
    // Visibility, mutability, `virtual`, and `override` are stored as
    // keyword tokens so `FunctionDef` can report them. Override bases
    // `(A, B)` keep ident and paren markers for `override_bases`.
    // Custom modifiers (`onlyOwner`) and base args (`Base(1)`) stay
    // skipped so the body still parses without one error per modifier.
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
                    || kind == SolidityKind::Constant.as_u16() =>
            {
                let Some(token) = cursor.bump() else {
                    break;
                };
                children.push(token_element(token));
            }
            Some(kind)
                if kind == SolidityKind::Virtual.as_u16()
                    || kind == SolidityKind::Override.as_u16() =>
            {
                let Some(token) = cursor.bump() else {
                    break;
                };
                children.push(token_element(token));
                if kind == SolidityKind::Override.as_u16()
                    && cursor.peek_kind() == Some(SolidityKind::LParen.as_u16())
                {
                    parse_override_bases(cursor, &mut children);
                }
            }
            Some(kind) if kind == SolidityKind::Ident.as_u16() => {
                cursor.bump();
                if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
                    skip_parens(cursor);
                }
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
    // Interface stub: no body, ends with `;`. Keeps the decl so hover
    // and completion still see the name.
    if cursor.peek_kind() == Some(SolidityKind::Semicolon.as_u16()) {
        let Some(semi) = cursor.bump() else {
            errors.push(ParseError::new(cursor.peek_span(), "expected `;`"));
            return None;
        };
        return Some(SyntaxNode::with_children(
            SolidityNode::Function.as_u16(),
            Span::new(start.start, semi.span.end),
            children,
        ));
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
    // Data location, payable, and indexed sit between type and name:
    // `string memory label`, `address payable holder`,
    // `address indexed from`. Consumed in any order so the trailing
    // name still parses.
    loop {
        match cursor.peek_kind() {
            Some(kind)
                if kind == SolidityKind::Memory.as_u16()
                    || kind == SolidityKind::Storage.as_u16()
                    || kind == SolidityKind::Calldata.as_u16()
                    || kind == SolidityKind::Payable.as_u16()
                    || kind == SolidityKind::Indexed.as_u16() =>
            {
                cursor.bump();
            }
            _ => break,
        }
    }
    let mut end = children
        .first()
        .and_then(|element| match element {
            SyntaxElement::Node(node) => Some(node.span.end),
            _ => None,
        })
        .unwrap_or(start);
    // Param names accept any word token, not only Ident: `from` lexes
    // as a keyword yet SBE uses it as `address from`. Structural tokens
    // never match, so nameless params like `uint,` stay nameless.
    if let Some(token) = cursor.peek().cloned()
        && is_param_name(&token.text)
    {
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

fn is_param_name(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
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
    let mut items: Vec<SyntaxElement> = Vec::new();
    loop {
        match cursor.peek_kind() {
            None => {
                errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
                return Some(SyntaxNode::with_children(
                    SolidityNode::Block.as_u16(),
                    Span::new(open.span.start, cursor.peek_span().end),
                    items,
                ));
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => break,
            Some(kind) if kind == SolidityKind::Assembly.as_u16() => {
                skip_assembly(cursor);
            }
            Some(kind) if kind == SolidityKind::LBrace.as_u16() => {
                if let Some(nested) = parse_block(cursor, errors) {
                    items.push(SyntaxElement::Node(nested));
                }
            }
            Some(kind) if kind == SolidityKind::If.as_u16() => {
                parse_if(cursor, errors, &mut items);
            }
            Some(kind) if kind == SolidityKind::For.as_u16() => {
                parse_for(cursor, errors, &mut items);
            }
            Some(kind) if kind == SolidityKind::While.as_u16() => {
                parse_while(cursor, errors, &mut items);
            }
            Some(kind) if kind == SolidityKind::Unchecked.as_u16() => {
                parse_unchecked(cursor, errors, &mut items);
            }
            Some(kind) if kind == SolidityKind::Try.as_u16() => {
                parse_try(cursor, errors, &mut items);
            }
            Some(kind) if kind == SolidityKind::LParen.as_u16() => match try_parse_tuple(cursor) {
                Some(nodes) => {
                    for node in nodes {
                        items.push(SyntaxElement::Node(node));
                    }
                }
                None => recover_statement(cursor),
            },
            Some(kind)
                if (is_type_start(kind) || kind == SolidityKind::Ident.as_u16())
                    && is_local_decl_ahead(cursor) =>
            {
                match parse_local_var(cursor) {
                    Ok(node) => items.push(SyntaxElement::Node(node)),
                    Err(error) => {
                        errors.push(error);
                        recover_statement(cursor);
                    }
                }
            }
            Some(kind) if kind == SolidityKind::Emit.as_u16() => {
                if let Some(emit) = try_parse_emit(cursor) {
                    items.push(SyntaxElement::Node(emit));
                } else {
                    recover_statement(cursor);
                }
            }
            Some(kind) if is_type_start(kind) || kind == SolidityKind::Ident.as_u16() => {
                if kind == SolidityKind::Ident.as_u16()
                    && let Some(member) = try_parse_member(cursor)
                {
                    items.push(SyntaxElement::Node(member));
                } else if kind == SolidityKind::Ident.as_u16()
                    && let Some(call) = try_parse_call(cursor)
                {
                    items.push(SyntaxElement::Node(call));
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
            items,
        ));
    };
    Some(SyntaxNode::with_children(
        SolidityNode::Block.as_u16(),
        Span::new(open.span.start, close.span.end),
        items,
    ))
}

/// Parse one control body into `items`: brace block or nested control.
///
/// Brace bodies recurse into `parse_block` and land as child `Block`
/// nodes. A nested control (`if` inside `if`) recurses the same way so
/// decls keep their scope. Anything else falls back to skipping one
/// statement with no node: single-statement bodies hold no decls in V1.
fn parse_control_body(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
    items: &mut Vec<SyntaxElement>,
) {
    match cursor.peek_kind() {
        Some(kind) if kind == SolidityKind::LBrace.as_u16() => {
            if let Some(body) = parse_block(cursor, errors) {
                items.push(SyntaxElement::Node(body));
            }
        }
        Some(kind) if kind == SolidityKind::If.as_u16() => parse_if(cursor, errors, items),
        Some(kind) if kind == SolidityKind::For.as_u16() => parse_for(cursor, errors, items),
        Some(kind) if kind == SolidityKind::While.as_u16() => parse_while(cursor, errors, items),
        Some(kind) if kind == SolidityKind::Unchecked.as_u16() => {
            parse_unchecked(cursor, errors, items);
        }
        Some(kind) if kind == SolidityKind::Try.as_u16() => parse_try(cursor, errors, items),
        Some(kind) if kind == SolidityKind::Assembly.as_u16() => skip_assembly(cursor),
        _ => recover_statement(cursor),
    }
}

/// Parse `if (cond) body [else body]`, pushing body blocks.
///
/// The condition is skipped balanced: only bodies hold decls. Broken
/// headers fall through so the body still parses without cascading.
fn parse_if(cursor: &mut Cursor, errors: &mut Vec<ParseError>, items: &mut Vec<SyntaxElement>) {
    cursor.bump();
    if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
        skip_parens(cursor);
    }
    parse_control_body(cursor, errors, items);
    if cursor.peek_kind() == Some(SolidityKind::Else.as_u16()) {
        cursor.bump();
        parse_control_body(cursor, errors, items);
    }
}

/// Parse `for (init; cond; post) body` as one bounded block.
///
/// The whole statement becomes one child `Block` from `for` to body end.
/// Header decls live inside it so uses after the loop miss. Bodies and
/// `all_*` still find decls through the wrapper.
fn parse_for(cursor: &mut Cursor, errors: &mut Vec<ParseError>, items: &mut Vec<SyntaxElement>) {
    let Some(for_token) = cursor.bump() else {
        return;
    };
    let start = for_token.span.start;
    let mut inner: Vec<SyntaxElement> = Vec::new();
    parse_for_header(cursor, &mut inner);
    parse_control_body(cursor, errors, &mut inner);
    let mut end = for_token.span.end;
    for element in &inner {
        if let SyntaxElement::Node(node) = element {
            end = end.max(node.span.end);
        }
    }
    // Cover skipped single-statement bodies with no node.
    if let Some(prev) = cursor.tokens.get(cursor.pos().saturating_sub(1)) {
        end = end.max(prev.span.end);
    }
    items.push(SyntaxElement::Node(SyntaxNode::with_children(
        SolidityNode::Block.as_u16(),
        Span::new(start, end),
        inner,
    )));
}

/// Parse `while (cond) body`, pushing the body block.
fn parse_while(cursor: &mut Cursor, errors: &mut Vec<ParseError>, items: &mut Vec<SyntaxElement>) {
    cursor.bump();
    if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
        skip_parens(cursor);
    }
    parse_control_body(cursor, errors, items);
}

/// Parse `unchecked { ... }`, pushing the body block.
fn parse_unchecked(
    cursor: &mut Cursor,
    errors: &mut Vec<ParseError>,
    items: &mut Vec<SyntaxElement>,
) {
    cursor.bump();
    if cursor.peek_kind() == Some(SolidityKind::LBrace.as_u16()) {
        parse_control_body(cursor, errors, items);
    } else {
        recover_statement(cursor);
    }
}

/// Parse `try callee [returns (...)] body (catch ... body)*`.
///
/// Bodies recurse as child blocks. Catch params stay deferred: the
/// parenthesized list is skipped balanced with no symbols, so catch
/// bindings never resolve in V1.
fn parse_try(cursor: &mut Cursor, errors: &mut Vec<ParseError>, items: &mut Vec<SyntaxElement>) {
    cursor.bump();
    loop {
        match cursor.peek_kind() {
            None => return,
            Some(kind) if kind == SolidityKind::LBrace.as_u16() => break,
            Some(kind)
                if kind == SolidityKind::Semicolon.as_u16()
                    || kind == SolidityKind::RBrace.as_u16() =>
            {
                return;
            }
            Some(kind) if kind == SolidityKind::LParen.as_u16() => skip_parens(cursor),
            _ => {
                cursor.bump();
            }
        }
    }
    parse_control_body(cursor, errors, items);
    loop {
        if cursor.peek_kind() != Some(SolidityKind::Catch.as_u16()) {
            return;
        }
        cursor.bump();
        if cursor.peek_kind() == Some(SolidityKind::Ident.as_u16()) {
            cursor.bump();
        }
        if cursor.peek_kind() == Some(SolidityKind::LParen.as_u16()) {
            skip_parens(cursor);
        }
        match cursor.peek_kind() {
            None => return,
            Some(kind)
                if kind == SolidityKind::RBrace.as_u16()
                    || kind == SolidityKind::Catch.as_u16() =>
            {
                return;
            }
            _ => parse_control_body(cursor, errors, items),
        }
    }
}

/// Parse the `(init; cond; post)` header, keeping an init declaration.
///
/// Balanced scan only: a `Type name [= init];` init becomes one LocalVar
/// node in the for wrapper so the body sees it but uses after the loop
/// miss. Anything else (empty init, assignment, unbalanced) stops
/// silently past `)`: no node, no error.
fn parse_for_header(cursor: &mut Cursor, items: &mut Vec<SyntaxElement>) {
    if cursor.peek_kind() != Some(SolidityKind::LParen.as_u16()) {
        return;
    }
    if !is_for_decl_ahead(cursor) {
        cursor.bump();
        skip_to_rparen(cursor);
        return;
    }
    cursor.bump();
    match parse_local_var(cursor) {
        Ok(node) => items.push(SyntaxElement::Node(node)),
        Err(_) => {
            skip_to_rparen(cursor);
            return;
        }
    }
    skip_to_rparen(cursor);
}

/// True when the `for (` init looks like `Type name [= ...];`.
fn is_for_decl_ahead(cursor: &mut Cursor) -> bool {
    let comment = SolidityKind::Comment.as_u16();
    let mut index = cursor.pos() + 1;
    while cursor
        .tokens
        .get(index)
        .is_some_and(|token| token.kind == comment)
    {
        index += 1;
    }
    let Some(first) = cursor.tokens.get(index) else {
        return false;
    };
    if !(is_type_start(first.kind) || first.kind == SolidityKind::Ident.as_u16()) {
        return false;
    }
    decl_ahead_at(cursor.tokens, index)
}

/// Skip to past the matching `)` at depth zero. Callers consumed `(`.
///
/// Stops at braces or EOF when unbalanced so body parsing survives.
fn skip_to_rparen(cursor: &mut Cursor) {
    let mut depth = 1u32;
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
        } else if kind == SolidityKind::LBrace.as_u16() || kind == SolidityKind::RBrace.as_u16() {
            return;
        }
        cursor.bump();
    }
}

/// Parse `(a, b, ...) = rhs;` tuple rows at block top level.
///
/// Each `Type name` element becomes one LocalVar node; bare ident
/// elements are pure assignments with no symbols. The row must close
/// with `)` then `=` and end with `;`: anything else returns `None`
/// with the cursor rewound so the caller recovers one statement.
fn try_parse_tuple(cursor: &mut Cursor) -> Option<Vec<SyntaxNode>> {
    let start = cursor.pos();
    if cursor.peek_kind() != Some(SolidityKind::LParen.as_u16()) {
        return None;
    }
    cursor.bump();
    let mut nodes = Vec::new();
    loop {
        if cursor.peek_kind() == Some(SolidityKind::RParen.as_u16()) {
            cursor.bump();
            break;
        }
        let element = cursor.pos();
        let Some(first) = cursor.peek().cloned() else {
            cursor.set_pos(start);
            return None;
        };
        if is_type_start(first.kind) {
            let Some(node) = parse_tuple_decl_element(cursor) else {
                cursor.set_pos(start);
                return None;
            };
            nodes.push(node);
        } else if first.kind == SolidityKind::Ident.as_u16() {
            if decl_ahead_at(cursor.tokens, element) {
                let Some(node) = parse_tuple_decl_element(cursor) else {
                    cursor.set_pos(start);
                    return None;
                };
                nodes.push(node);
            } else if is_param_name(&first.text) {
                cursor.bump();
            } else {
                cursor.set_pos(start);
                return None;
            }
        } else {
            cursor.set_pos(start);
            return None;
        }
        match cursor.peek_kind() {
            Some(kind) if kind == SolidityKind::Comma.as_u16() => {
                cursor.bump();
            }
            Some(kind) if kind == SolidityKind::RParen.as_u16() => {
                cursor.bump();
                break;
            }
            _ => {
                cursor.set_pos(start);
                return None;
            }
        }
    }
    if cursor.peek_kind() != Some(SolidityKind::Assign.as_u16()) {
        cursor.set_pos(start);
        return None;
    }
    cursor.bump();
    loop {
        match cursor.peek_kind() {
            None => {
                cursor.set_pos(start);
                return None;
            }
            Some(kind) if kind == SolidityKind::Semicolon.as_u16() => {
                cursor.bump();
                return Some(nodes);
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => {
                cursor.set_pos(start);
                return None;
            }
            Some(kind) if kind == SolidityKind::LBrace.as_u16() => skip_balanced(cursor),
            _ => {
                cursor.bump();
            }
        }
    }
}

/// Parse one `Type [dataloc] name` tuple element into a LocalVar node.
///
/// Shares `parse_type` shape with `parse_var` but spans the element
/// only: the shared `= rhs` belongs to the row, not to one name.
fn parse_tuple_decl_element(cursor: &mut Cursor) -> Option<SyntaxNode> {
    let var_type = parse_type(cursor).ok()?;
    let start = var_type.span.start;
    loop {
        match cursor.peek_kind() {
            Some(kind)
                if kind == SolidityKind::Memory.as_u16()
                    || kind == SolidityKind::Storage.as_u16()
                    || kind == SolidityKind::Calldata.as_u16() =>
            {
                cursor.bump();
            }
            _ => break,
        }
    }
    let name = cursor.peek().cloned()?;
    if !is_param_name(&name.text) {
        return None;
    }
    cursor.bump();
    let end = name.span.end.max(var_type.span.end);
    Some(SyntaxNode::with_children(
        SolidityNode::LocalVar.as_u16(),
        Span::new(start, end),
        vec![SyntaxElement::Node(var_type), token_element(name.clone())],
    ))
}

/// Scan `{...}(...)` options plus call parens starting at `lbrace`.
///
/// Returns the brace pair, the call markers (`)` alone for empty, `(` plus
/// commas otherwise), the closing paren, and its index. `None` on any
/// failure so callers fall back to plain.
fn scan_options_call(
    tokens: &[forgels_syntax::Token],
    lbrace_pos: usize,
) -> Option<(
    forgels_syntax::Token,
    forgels_syntax::Token,
    Vec<forgels_syntax::Token>,
    forgels_syntax::Token,
    usize,
)> {
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
    let lbrace = tokens.get(lbrace_pos)?.clone();
    if lbrace.kind != SolidityKind::LBrace.as_u16() {
        return None;
    }
    let mut depth = 0u32;
    let mut rbrace_pos = None;
    let mut index = lbrace_pos;
    while let Some(token) = tokens.get(index) {
        if token.kind == comment {
            index += 1;
            continue;
        }
        if token.kind == SolidityKind::LBrace.as_u16() {
            depth += 1;
        } else if token.kind == SolidityKind::RBrace.as_u16() {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                rbrace_pos = Some(index);
                break;
            }
        }
        index += 1;
    }
    let rbrace_pos = rbrace_pos?;
    let rbrace = tokens.get(rbrace_pos)?.clone();
    let lparen_pos = next_code(rbrace_pos + 1)?;
    let lparen = tokens.get(lparen_pos)?.clone();
    if lparen.kind != SolidityKind::LParen.as_u16() {
        return None;
    }
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut commas = Vec::new();
    let mut found_close = None;
    let mut scan = lparen_pos;
    while let Some(token) = tokens.get(scan) {
        if token.kind == comment {
            scan += 1;
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
                found_close = Some(scan);
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
        scan += 1;
    }
    let close_pos = found_close?;
    let rparen = tokens.get(close_pos)?.clone();
    let after_lparen = next_code(lparen_pos + 1);
    let is_empty = after_lparen.is_some_and(|at| at == close_pos);
    let markers = if is_empty {
        vec![rparen.clone()]
    } else {
        let mut out = vec![lparen.clone()];
        out.extend(commas);
        out
    };
    Some((lbrace, rbrace, markers, rparen, close_pos))
}

/// Try `recv.member` with single ident receiver, including `this`/`super`.
///
/// Returns the Member node and consumes through the member name (or dot
/// when the name is still missing). Only the first dot is stored; chained
/// tails like `.c` in `a.b.c` stay for `recover_statement`. Calls add
/// markers: `(` plus commas for non-empty, `)` alone for empty. Options
/// like `{value: 1}` add `{` and `}` markers when balanced and followed
/// by parens. Span covers `recv` through `)` for calls, `recv` through
/// member for plain. Unclosed or braced call suffixes fall back to plain.
pub(crate) fn try_parse_member(cursor: &mut Cursor) -> Option<SyntaxNode> {
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
    let recv_idx = next_code(cursor.pos())?;
    let recv = tokens.get(recv_idx)?.clone();
    if recv.kind != SolidityKind::Ident.as_u16() {
        return None;
    }
    let dot_idx = next_code(recv_idx + 1)?;
    let dot = tokens.get(dot_idx)?.clone();
    if dot.kind != SolidityKind::Dot.as_u16() {
        return None;
    }
    let member_idx = next_code(dot_idx + 1);
    let member = member_idx
        .and_then(|index| tokens.get(index))
        .filter(|token| token.kind == SolidityKind::Ident.as_u16())
        .cloned();
    let start = recv.span.start;
    let finish_plain =
        |cursor: &mut Cursor, children: Vec<SyntaxElement>, end: u32, target: usize| {
            while cursor.pos() <= target {
                if cursor.bump().is_none() {
                    break;
                }
            }
            Some(SyntaxNode::with_children(
                SolidityNode::Member.as_u16(),
                Span::new(start, end),
                children,
            ))
        };
    let Some(name) = member.clone() else {
        let children = vec![token_element(recv), token_element(dot.clone())];
        return finish_plain(cursor, children, dot.span.end, dot_idx);
    };
    let member_pos = member_idx.unwrap_or(dot_idx);
    let mut base = vec![token_element(recv), token_element(dot.clone())];
    base.push(token_element(name.clone()));
    let plain_end = name.span.end;
    let Some(after_pos) = next_code(member_pos + 1) else {
        return finish_plain(cursor, base, plain_end, member_pos);
    };
    let after = tokens.get(after_pos)?.clone();
    if after.kind == SolidityKind::LBrace.as_u16() {
        let Some((open, close, call_markers, rparen, close_pos)) =
            scan_options_call(tokens, after_pos)
        else {
            return finish_plain(cursor, base, plain_end, member_pos);
        };
        let mut children = base;
        children.push(token_element(open));
        children.push(token_element(close));
        for marker in call_markers {
            children.push(token_element(marker));
        }
        while cursor.pos() <= close_pos {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(SyntaxNode::with_children(
            SolidityNode::Member.as_u16(),
            Span::new(start, rparen.span.end),
            children,
        ));
    }
    if after.kind != SolidityKind::LParen.as_u16() {
        return finish_plain(cursor, base, plain_end, member_pos);
    }
    let lparen_pos = after_pos;
    let lparen = after;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut commas = Vec::new();
    let mut rparen_pos = None;
    let mut index = lparen_pos;
    while let Some(token) = tokens.get(index) {
        if token.kind == comment {
            index += 1;
            continue;
        }
        let kind = token.kind;
        if kind == SolidityKind::LBrace.as_u16() {
            return finish_plain(cursor, base, plain_end, member_pos);
        }
        if kind == SolidityKind::LParen.as_u16() {
            paren_depth += 1;
        } else if kind == SolidityKind::RParen.as_u16() {
            if paren_depth == 0 {
                return finish_plain(cursor, base, plain_end, member_pos);
            }
            paren_depth -= 1;
            if paren_depth == 0 {
                rparen_pos = Some(index);
                break;
            }
        } else if kind == SolidityKind::LBracket.as_u16() {
            bracket_depth += 1;
        } else if kind == SolidityKind::RBracket.as_u16() {
            if bracket_depth == 0 {
                return finish_plain(cursor, base, plain_end, member_pos);
            }
            bracket_depth -= 1;
        } else if kind == SolidityKind::Comma.as_u16() && paren_depth == 1 && bracket_depth == 0 {
            commas.push(token.clone());
        }
        index += 1;
    }
    let close_pos = match rparen_pos {
        Some(pos) => pos,
        None => return finish_plain(cursor, base, plain_end, member_pos),
    };
    let rparen = tokens.get(close_pos)?.clone();
    let after_lparen = next_code(lparen_pos + 1);
    let is_empty = after_lparen.is_some_and(|at| at == close_pos);
    let mut children = base;
    if is_empty {
        children.push(token_element(rparen.clone()));
    } else {
        children.push(token_element(lparen));
        for comma in commas {
            children.push(token_element(comma));
        }
    }
    while cursor.pos() <= close_pos {
        if cursor.bump().is_none() {
            break;
        }
    }
    Some(SyntaxNode::with_children(
        SolidityNode::Member.as_u16(),
        Span::new(start, rparen.span.end),
        children,
    ))
}

/// Try `name(args)` at block top level. Returns the Call node on success.
///
/// Plain calls plus `{value: 1}` options: options need balanced braces
/// followed by parens and store `{` and `}` markers with the span.
/// Member access (`a.b()`), named args (`f({x: 1})`), unclosed options,
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
    let after_idx = next_code(callee_idx + 1)?;
    let after = tokens.get(after_idx)?.clone();
    if after.kind == SolidityKind::LBrace.as_u16() {
        let (open, close, markers, rparen, close_pos) = scan_options_call(tokens, after_idx)?;
        let mut children = vec![token_element(callee.clone())];
        children.push(token_element(open));
        children.push(token_element(close));
        let single_paren = markers.len() == 1
            && markers
                .first()
                .is_some_and(|token| token.kind == SolidityKind::RParen.as_u16());
        for marker in markers {
            // Empty options calls keep only braces; the lone `)` marker
            // is dropped so `arg_count` stays zero.
            if single_paren && marker.kind == SolidityKind::RParen.as_u16() {
                continue;
            }
            children.push(token_element(marker));
        }
        let node = SyntaxNode::with_children(
            SolidityNode::Call.as_u16(),
            Span::new(callee.span.start, rparen.span.end),
            children,
        );
        while cursor.pos() <= close_pos {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(node);
    }
    if after.kind != SolidityKind::LParen.as_u16() {
        return None;
    }
    let lparen_idx = after_idx;
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

/// Try `emit Name(args)` at block top level. Returns the Emit node.
///
/// Partial rows keep what exists: bare `emit`, `emit Name`, and
/// unclosed `emit Name(args` all yield a node so typing keeps working.
/// Full rows count commas at paren depth one; empty parens give zero.
pub(crate) fn try_parse_emit(cursor: &mut Cursor) -> Option<SyntaxNode> {
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
    let emit_idx = next_code(cursor.pos())?;
    let emit = tokens.get(emit_idx)?.clone();
    if emit.kind != SolidityKind::Emit.as_u16() {
        return None;
    }
    let Some(name_idx) = next_code(emit_idx + 1) else {
        let node = SyntaxNode::with_children(
            SolidityNode::Emit.as_u16(),
            Span::new(emit.span.start, emit.span.end),
            vec![token_element(emit)],
        );
        while cursor.pos() <= emit_idx {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(node);
    };
    let name = tokens.get(name_idx)?.clone();
    if name.kind != SolidityKind::Ident.as_u16() {
        let node = SyntaxNode::with_children(
            SolidityNode::Emit.as_u16(),
            Span::new(emit.span.start, emit.span.end),
            vec![token_element(emit)],
        );
        while cursor.pos() <= emit_idx {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(node);
    }
    let Some(lparen_idx) = next_code(name_idx + 1) else {
        let node = SyntaxNode::with_children(
            SolidityNode::Emit.as_u16(),
            Span::new(emit.span.start, name.span.end),
            vec![token_element(emit), token_element(name)],
        );
        while cursor.pos() <= name_idx {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(node);
    };
    let lparen = tokens.get(lparen_idx)?.clone();
    if lparen.kind != SolidityKind::LParen.as_u16() {
        let node = SyntaxNode::with_children(
            SolidityNode::Emit.as_u16(),
            Span::new(emit.span.start, name.span.end),
            vec![token_element(emit), token_element(name)],
        );
        while cursor.pos() <= name_idx {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(node);
    }
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut commas = Vec::new();
    let mut rparen_idx: Option<usize> = None;
    let mut scan_end = lparen_idx;
    let mut index = lparen_idx;
    let mut closed = false;
    while let Some(token) = tokens.get(index) {
        if token.kind == comment {
            index += 1;
            continue;
        }
        let kind = token.kind;
        if kind == SolidityKind::LBrace.as_u16() {
            break;
        }
        if kind == SolidityKind::LParen.as_u16() {
            paren_depth += 1;
        } else if kind == SolidityKind::RParen.as_u16() {
            if paren_depth == 0 {
                break;
            }
            paren_depth -= 1;
            if paren_depth == 0 {
                rparen_idx = Some(index);
                scan_end = index;
                closed = true;
                break;
            }
        } else if kind == SolidityKind::LBracket.as_u16() {
            bracket_depth += 1;
        } else if kind == SolidityKind::RBracket.as_u16() {
            if bracket_depth == 0 {
                break;
            }
            bracket_depth -= 1;
        } else if kind == SolidityKind::Comma.as_u16() && paren_depth == 1 && bracket_depth == 0 {
            commas.push(token.clone());
        } else if (kind == SolidityKind::Semicolon.as_u16() && paren_depth == 1)
            || (kind == SolidityKind::RBrace.as_u16() && paren_depth > 0)
        {
            break;
        }
        scan_end = index;
        index += 1;
    }
    if closed {
        let close_idx = rparen_idx?;
        let rparen = tokens.get(close_idx)?.clone();
        let after_lparen = next_code(lparen_idx + 1);
        let is_empty = after_lparen.is_none_or(|at| at == close_idx);
        let start = emit.span.start;
        let end = rparen.span.end;
        let mut children = vec![token_element(emit), token_element(name)];
        if !is_empty {
            children.push(token_element(lparen));
            for comma in commas {
                children.push(token_element(comma));
            }
        }
        let node =
            SyntaxNode::with_children(SolidityNode::Emit.as_u16(), Span::new(start, end), children);
        while cursor.pos() <= close_idx {
            if cursor.bump().is_none() {
                break;
            }
        }
        return Some(node);
    }
    let after_lparen = next_code(lparen_idx + 1);
    let has_content = after_lparen.is_some_and(|at| at <= scan_end);
    let start = emit.span.start;
    let name_end = name.span.end;
    let mut children = vec![token_element(emit), token_element(name)];
    if has_content {
        children.push(token_element(lparen));
        for comma in commas {
            children.push(token_element(comma));
        }
    }
    let end = tokens
        .get(scan_end)
        .map(|token| token.span.end)
        .unwrap_or(name_end)
        .max(name_end);
    let target = scan_end.max(name_idx);
    let node =
        SyntaxNode::with_children(SolidityNode::Emit.as_u16(), Span::new(start, end), children);
    while cursor.pos() <= target {
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

/// Parse `using L for T;`, `using L for *;`, and `using L for T global;`.
///
/// Operator forms (`using {f as +}`) and multi-library lists stay
/// deferred: the full row is consumed with no error and a span-only
/// node so unknown stays silent while analysis skips them.
pub(crate) fn parse_using(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let Some(using) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected using"));
        return None;
    };
    let start = using.span.start;
    let mut children = vec![token_element(using)];
    if cursor.peek_kind() == Some(SolidityKind::LBrace.as_u16()) {
        let end = skip_to_semi(cursor);
        return Some(SyntaxNode::with_children(
            SolidityNode::Using.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    let library = match cursor.peek_kind() {
        Some(kind) if kind == SolidityKind::Ident.as_u16() => cursor.bump(),
        _ => {
            errors.push(ParseError::new(cursor.peek_span(), "expected library name"));
            skip_to_semi(cursor);
            return None;
        }
    };
    let Some(library) = library else {
        errors.push(ParseError::new(cursor.peek_span(), "expected library name"));
        return None;
    };
    children.push(token_element(library));
    if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
        let end = skip_to_semi(cursor);
        return Some(SyntaxNode::with_children(
            SolidityNode::Using.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    let Some(for_token) = cursor.bump() else {
        errors.push(ParseError::new(cursor.peek_span(), "expected `for`"));
        return None;
    };
    if for_token.kind != SolidityKind::For.as_u16() {
        errors.push(ParseError::new(for_token.span, "expected `for`"));
        let end = skip_to_semi(cursor);
        return Some(SyntaxNode::with_children(
            SolidityNode::Using.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    children.push(token_element(for_token));
    if cursor.peek_kind() == Some(SolidityKind::Star.as_u16()) {
        let Some(star) = cursor.bump() else {
            errors.push(ParseError::new(cursor.peek_span(), "expected target"));
            return None;
        };
        children.push(token_element(star));
    } else if let Some(kind) = cursor.peek_kind()
        && (is_type_start(kind) || kind == SolidityKind::Ident.as_u16())
    {
        match parse_type(cursor) {
            Ok(target) => children.push(SyntaxElement::Node(target)),
            Err(error) => {
                errors.push(error);
                let end = skip_to_semi(cursor);
                return Some(SyntaxNode::with_children(
                    SolidityNode::Using.as_u16(),
                    Span::new(start, end),
                    children,
                ));
            }
        }
    } else {
        errors.push(ParseError::new(cursor.peek_span(), "expected target type"));
        let end = skip_to_semi(cursor);
        return Some(SyntaxNode::with_children(
            SolidityNode::Using.as_u16(),
            Span::new(start, end),
            children,
        ));
    }
    if cursor.peek_kind() == Some(SolidityKind::Global.as_u16())
        && let Some(global) = cursor.bump()
    {
        children.push(token_element(global));
    }
    match cursor.expect(SolidityKind::Semicolon, "`;`") {
        Ok(semi) => {
            let end = semi.span.end;
            children.push(token_element(semi));
            Some(SyntaxNode::with_children(
                SolidityNode::Using.as_u16(),
                Span::new(start, end),
                children,
            ))
        }
        Err(error) => {
            errors.push(error);
            let end = cursor.peek_span().end.max(start);
            Some(SyntaxNode::with_children(
                SolidityNode::Using.as_u16(),
                Span::new(start, end),
                children,
            ))
        }
    }
}

fn skip_to_semi(cursor: &mut Cursor) -> u32 {
    let mut depth = 0u32;
    let mut end = cursor.peek_span().end;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LBrace.as_u16() {
            depth += 1;
        } else if kind == SolidityKind::RBrace.as_u16() {
            if depth == 0 {
                break;
            }
            depth -= 1;
        } else if kind == SolidityKind::Semicolon.as_u16() && depth == 0 {
            if let Some(semi) = cursor.bump() {
                end = semi.span.end;
            }
            return end;
        }
        if let Some(bumped) = cursor.bump() {
            end = bumped.span.end;
        } else {
            break;
        }
    }
    end
}

/// Store `override(A, B)` base markers: parens, idents, and commas.
///
/// Stops at `)` or at any non-base token so broken lists never hang
/// the function header. Callers already checked the opening paren.
fn parse_override_bases(cursor: &mut Cursor, children: &mut Vec<SyntaxElement>) {
    let Some(open) = cursor.bump() else {
        return;
    };
    children.push(token_element(open));
    loop {
        match cursor.peek_kind() {
            Some(kind) if kind == SolidityKind::Ident.as_u16() => {
                let Some(name) = cursor.bump() else {
                    return;
                };
                children.push(token_element(name));
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    let Some(comma) = cursor.bump() else {
                        return;
                    };
                    children.push(token_element(comma));
                }
            }
            Some(kind) if kind == SolidityKind::RParen.as_u16() => {
                let Some(close) = cursor.bump() else {
                    return;
                };
                children.push(token_element(close));
                return;
            }
            _ => return,
        }
    }
}

/// True when the upcoming tokens look like `Type name [= ;]`.
pub(crate) fn is_local_decl_ahead(cursor: &mut Cursor) -> bool {
    decl_ahead_at(cursor.tokens, cursor.pos())
}

fn decl_ahead_at(tokens: &[forgels_syntax::Token], start: usize) -> bool {
    let tokens = match tokens.get(start..) {
        Some(slice) => slice,
        None => return false,
    };
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
    // Data location and modifiers sit between type and name:
    // `bytes memory x`, `address payable holder`, `uint public y`.
    // Skipped in a loop so the trailing name still parses.
    loop {
        if tokens.get(index).is_some_and(|token| {
            token.kind == SolidityKind::Memory.as_u16()
                || token.kind == SolidityKind::Storage.as_u16()
                || token.kind == SolidityKind::Calldata.as_u16()
                || token.kind == SolidityKind::Public.as_u16()
                || token.kind == SolidityKind::Private.as_u16()
                || token.kind == SolidityKind::Internal.as_u16()
                || token.kind == SolidityKind::Constant.as_u16()
                || token.kind == SolidityKind::Immutable.as_u16()
                || token.kind == SolidityKind::Override.as_u16()
                || token.kind == SolidityKind::Transient.as_u16()
                || token.kind == SolidityKind::Payable.as_u16()
        }) {
            index += 1;
        } else {
            break;
        }
    }
    // Names accept any word token, matching `parse_var` and `parse_param`:
    // `after` lexes as reserved but works as `uint after`.
    tokens
        .get(index)
        .is_some_and(|token| is_param_name(&token.text))
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
