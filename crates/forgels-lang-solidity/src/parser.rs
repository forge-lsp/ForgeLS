use crate::SolidityLexer;
use crate::lexer::SolidityKind;
use crate::{SolidityFile, SolidityNode};
use forgels_syntax::{
    Lexer, ParseError, ParseResult, Parser, Span, SyntaxElement, SyntaxNode, Token,
};

#[derive(Debug, Default)]
pub struct SolidityParser;

impl Parser for SolidityParser {
    type Output = SolidityFile;

    fn parse(&self, tokens: &[Token]) -> ParseResult<Self::Output> {
        let (file, mut errors) = self.parse_with_errors(tokens);
        if let Some(first) = errors.drain(..).next() {
            return Err(first);
        }
        Ok(file)
    }

    fn parse_text(&self, text: &str) -> ParseResult<Self::Output> {
        let lexer = SolidityLexer;
        let tokens = lexer.tokenize(text);
        self.parse(&tokens)
    }
}

impl SolidityParser {
    /// Parse the skeleton, collecting every error with recovery.
    ///
    /// Always returns a file, even when empty. Callers map the error list
    /// one to one into diagnostics.
    pub fn parse_with_errors(&self, tokens: &[Token]) -> (SolidityFile, Vec<ParseError>) {
        let file_end = tokens.last().map(|token| token.span.end).unwrap_or(0);
        let mut cursor = Cursor::new(tokens, file_end);
        let mut items: Vec<SyntaxElement> = Vec::new();
        let mut errors: Vec<ParseError> = Vec::new();
        while !cursor.at_end() {
            let top = cursor.pos();
            match cursor.peek_kind() {
                Some(kind) if kind == SolidityKind::Pragma.as_u16() => {
                    match parse_pragma(&mut cursor) {
                        Ok(node) => items.push(SyntaxElement::Node(node)),
                        Err(error) => {
                            errors.push(error);
                            cursor.recover();
                        }
                    }
                }
                Some(kind) if kind == SolidityKind::Import.as_u16() => {
                    match parse_import(&mut cursor) {
                        Ok(node) => items.push(SyntaxElement::Node(node)),
                        Err(error) => {
                            errors.push(error);
                            cursor.recover();
                        }
                    }
                }
                Some(kind) if kind == SolidityKind::Contract.as_u16() => {
                    match parse_contract(&mut cursor, &mut errors) {
                        Some(node) => items.push(SyntaxElement::Node(node)),
                        None => cursor.recover(),
                    }
                }
                Some(kind) if kind == SolidityKind::Error.as_u16() => {
                    let token = cursor.bump().expect("peeked token exists");
                    errors.push(ParseError::new(token.span, "unexpected character"));
                }
                Some(_) => {
                    let span = cursor.peek_span();
                    errors.push(ParseError::new(
                        span,
                        "expected pragma, import, or contract",
                    ));
                    cursor.recover();
                }
                None => break,
            }
            // Recovery must always move forward; never spin in place.
            if cursor.pos() == top {
                cursor.bump();
            }
        }
        let file = SolidityFile::new(SyntaxNode::with_children(
            SolidityNode::File.as_u16(),
            Span::new(0, file_end),
            items,
        ));
        (file, errors)
    }
}

/// Token cursor that hides comments: the parser never sees trivia.
struct Cursor<'a> {
    tokens: &'a [Token],
    pos: usize,
    eof: Span,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [Token], file_end: u32) -> Self {
        Self {
            tokens,
            pos: 0,
            eof: Span::new(file_end, file_end),
        }
    }

    fn pos(&self) -> usize {
        self.pos
    }

    fn at_end(&mut self) -> bool {
        self.peek().is_none()
    }

    fn peek(&mut self) -> Option<&'a Token> {
        while let Some(token) = self.tokens.get(self.pos) {
            if token.kind == crate::lexer::SolidityKind::Comment.as_u16() {
                self.pos += 1;
            } else {
                return Some(token);
            }
        }
        None
    }

    fn peek_kind(&mut self) -> Option<u16> {
        self.peek().map(|token| token.kind)
    }

    fn peek_span(&mut self) -> Span {
        self.peek().map(|token| token.span).unwrap_or(self.eof)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        self.pos += 1;
        Some(token)
    }

    fn expect(&mut self, kind: SolidityKind, what: &str) -> Result<Token, ParseError> {
        match self.peek_kind() {
            Some(found) if found == kind.as_u16() => Ok(self.bump().expect("peeked token exists")),
            _ => Err(ParseError::new(
                self.peek_span(),
                format!("expected {what}"),
            )),
        }
    }

    /// Skip to the next sync point: past `;`, or stop before a top-level
    /// keyword, `}`, or EOF. Always moves forward.
    fn recover(&mut self) {
        while let Some(kind) = self.peek_kind() {
            if kind == SolidityKind::Semicolon.as_u16() {
                self.bump();
                return;
            }
            if matches!(
                kind,
                k if k == SolidityKind::Pragma.as_u16()
                    || k == SolidityKind::Import.as_u16()
                    || k == SolidityKind::Contract.as_u16()
                    || k == SolidityKind::RBrace.as_u16()
            ) {
                return;
            }
            self.bump();
        }
    }
}

fn token_element(token: Token) -> SyntaxElement {
    SyntaxElement::Token(forgels_syntax::SyntaxToken {
        kind: token.kind,
        text: token.text,
        span: token.span,
    })
}

fn parse_pragma(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start = cursor.bump().expect("pragma keyword exists").span;
    let name = cursor.expect(SolidityKind::Ident, "pragma name")?;
    let mut children = vec![token_element(name)];
    let mut saw_version = false;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::Semicolon.as_u16()
            || kind == SolidityKind::Pragma.as_u16()
            || kind == SolidityKind::Import.as_u16()
            || kind == SolidityKind::Contract.as_u16()
            || kind == SolidityKind::RBrace.as_u16()
        {
            break;
        }
        let token = cursor.bump().expect("peeked token exists");
        children.push(token_element(token));
        saw_version = true;
    }
    if !saw_version {
        return Err(ParseError::new(
            cursor.peek_span(),
            "expected pragma version",
        ));
    }
    let end = cursor.expect(SolidityKind::Semicolon, "`;`")?;
    Ok(SyntaxNode::with_children(
        SolidityNode::Pragma.as_u16(),
        Span::new(start.start, end.span.end),
        children,
    ))
}

fn parse_import(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start = cursor.bump().expect("import keyword exists").span;
    let mut names: Vec<SyntaxElement> = Vec::new();
    let path: Option<Token>;
    match cursor.peek_kind() {
        Some(kind) if kind == SolidityKind::StringLit.as_u16() => {
            path = cursor.bump();
            if cursor.peek_kind() == Some(SolidityKind::As.as_u16()) {
                cursor.bump();
                let alias = cursor.expect(SolidityKind::Ident, "import alias")?;
                names.push(token_element(alias));
            }
        }
        Some(kind) if kind == SolidityKind::LBrace.as_u16() => {
            cursor.bump();
            loop {
                let name = cursor.expect(SolidityKind::Ident, "imported name")?;
                names.push(token_element(name));
                if cursor.peek_kind() == Some(SolidityKind::As.as_u16()) {
                    cursor.bump();
                    let alias = cursor.expect(SolidityKind::Ident, "import alias")?;
                    names.push(token_element(alias));
                }
                if cursor.peek_kind() == Some(SolidityKind::Comma.as_u16()) {
                    cursor.bump();
                } else {
                    break;
                }
            }
            cursor.expect(SolidityKind::RBrace, "`}`")?;
            cursor.expect(SolidityKind::From, "`from`")?;
            let found = cursor.expect(SolidityKind::StringLit, "import path")?;
            path = Some(found);
        }
        Some(kind) if kind == SolidityKind::Star.as_u16() => {
            cursor.bump();
            cursor.expect(SolidityKind::As, "`as`")?;
            let alias = cursor.expect(SolidityKind::Ident, "import alias")?;
            names.push(token_element(alias));
            cursor.expect(SolidityKind::From, "`from`")?;
            let found = cursor.expect(SolidityKind::StringLit, "import path")?;
            path = Some(found);
        }
        _ => {
            return Err(ParseError::new(
                cursor.peek_span(),
                "expected import path or symbol list",
            ));
        }
    }
    let Some(path) = path else {
        return Err(ParseError::new(cursor.peek_span(), "expected import path"));
    };
    names.push(token_element(path));
    let end = cursor.expect(SolidityKind::Semicolon, "`;`")?;
    Ok(SyntaxNode::with_children(
        SolidityNode::Import.as_u16(),
        Span::new(start.start, end.span.end),
        names,
    ))
}

fn parse_contract(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let start = cursor.bump().expect("contract keyword exists").span;
    let name = match cursor.expect(SolidityKind::Ident, "contract name") {
        Ok(name) => name,
        Err(error) => {
            errors.push(error);
            return None;
        }
    };
    // Heritage and modifiers belong to a later leaf. Skip to the body.
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::LBrace.as_u16() || kind == SolidityKind::RBrace.as_u16() {
            break;
        }
        if kind == SolidityKind::Pragma.as_u16()
            || kind == SolidityKind::Import.as_u16()
            || kind == SolidityKind::Contract.as_u16()
        {
            break;
        }
        cursor.bump();
    }
    if let Err(error) = cursor.expect(SolidityKind::LBrace, "`{`") {
        errors.push(error);
        return None;
    }
    let mut members: Vec<SyntaxElement> = vec![token_element(name)];
    loop {
        match cursor.peek_kind() {
            None => {
                errors.push(ParseError::new(cursor.peek_span(), "unclosed `{`"));
                return None;
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => break,
            Some(kind)
                if kind == SolidityKind::Function.as_u16()
                    || kind == SolidityKind::Constructor.as_u16() =>
            {
                match parse_function(cursor, errors) {
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
    let close = cursor.bump().expect("closing brace exists");
    Some(SyntaxNode::with_children(
        SolidityNode::Contract.as_u16(),
        Span::new(start.start, close.span.end),
        members,
    ))
}

/// Skip to the next member, brace aware: a `}` only stops the search at
/// depth zero. Always moves forward.
fn recover_member(cursor: &mut Cursor) {
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
                || is_type_start(kind)
                || kind == SolidityKind::Ident.as_u16()
            {
                return;
            }
        }
        cursor.bump();
    }
}

/// True when a token kind can start a type: elementary, sized, mapping,
/// or a user type name.
fn is_type_start(kind: u16) -> bool {
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
    ];
    TYPES.iter().any(|type_kind| type_kind.as_u16() == kind)
}

/// Parse `Type`, `Type[]`, `Type[N]`, and `mapping(K => V)` with suffixes.
fn parse_type(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start_span = cursor.peek_span();
    let mut parts: Vec<SyntaxElement> = Vec::new();
    if cursor.peek_kind() == Some(SolidityKind::Mapping.as_u16()) {
        let mapping = cursor.bump().expect("mapping keyword exists");
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
        let first = cursor.bump().expect("type token exists");
        parts.push(token_element(first));
    } else if cursor.peek_kind() == Some(SolidityKind::Ident.as_u16()) {
        // User type outside the beta subset: keep the name, flag nothing.
        let name = cursor.bump().expect("type name exists");
        parts.push(token_element(name));
    } else {
        return Err(ParseError::new(cursor.peek_span(), "expected type"));
    }
    while cursor.peek_kind() == Some(SolidityKind::LBracket.as_u16()) {
        let open = cursor.bump().expect("bracket exists");
        parts.push(token_element(open));
        if cursor.peek_kind() == Some(SolidityKind::Number.as_u16()) {
            let size = cursor.bump().expect("array size exists");
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

fn parse_mapping_key(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    parse_type_value(cursor)
}

fn parse_type_value(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    match cursor.peek_kind() {
        Some(kind) if is_type_start(kind) || kind == SolidityKind::Ident.as_u16() => {
            parse_type(cursor)
        }
        _ => Err(ParseError::new(cursor.peek_span(), "expected type")),
    }
}

/// Parse `Type name [= init] ;` at contract level.
fn parse_state_var(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    parse_var(cursor, SolidityNode::StateVar)
}

/// Parse `Type name [= init] ;` inside a block.
fn parse_local_var(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    parse_var(cursor, SolidityNode::LocalVar)
}

fn parse_var(cursor: &mut Cursor, kind: SolidityNode) -> Result<SyntaxNode, ParseError> {
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
fn parse_function(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
    let head = cursor.bump().expect("function keyword exists");
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
fn parse_param(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
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
        let name = cursor.bump().expect("param name exists");
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
/// Local errors go to `errors` while valid locals stay. `None` means the
/// block never closed.
fn parse_block(cursor: &mut Cursor, errors: &mut Vec<ParseError>) -> Option<SyntaxNode> {
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
                return None;
            }
            Some(kind) if kind == SolidityKind::RBrace.as_u16() => break,
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
                // Type-like token without a following name: an expression
                // statement, skipped like any other.
                recover_statement(cursor);
            }
            _ => {
                recover_statement(cursor);
            }
        }
    }
    let close = cursor.bump().expect("closing brace exists");
    Some(SyntaxNode::with_children(
        SolidityNode::Block.as_u16(),
        Span::new(open.span.start, close.span.end),
        locals,
    ))
}

/// True when the upcoming tokens look like `Type name [= ;]`.
fn is_local_decl_ahead(cursor: &mut Cursor) -> bool {
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
fn recover_statement(cursor: &mut Cursor) {
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

/// Skip a balanced `{ ... }` starting at the opening brace.
fn skip_balanced(cursor: &mut Cursor) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(text: &str) -> SolidityFile {
        SolidityParser
            .parse_text(text)
            .expect("valid skeleton parses")
    }

    #[test]
    fn valid_skeleton_parses_without_error() {
        let text =
            "pragma solidity ^0.8.0;\nimport {A as B, C} from \"./A.sol\";\ncontract Token {}\n";
        let file = parse_ok(text);
        assert_eq!(file.pragmas().len(), 1);
        assert_eq!(file.imports().len(), 1);
        assert_eq!(file.contracts().len(), 1);
        assert_eq!(file.pragmas()[0].name().as_deref(), Some("solidity"));
        assert_eq!(file.imports()[0].path().as_deref(), Some("./A.sol"));
        assert_eq!(
            file.imports()[0].symbols(),
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
        assert_eq!(file.contracts()[0].name().as_deref(), Some("Token"));
    }

    #[test]
    fn all_import_forms_parse() {
        let text =
            "import \"./A.sol\";\nimport \"./B.sol\" as NS;\nimport * as All from \"./C.sol\";\n";
        let file = parse_ok(text);
        assert_eq!(file.imports().len(), 3);
        assert_eq!(file.imports()[0].path().as_deref(), Some("./A.sol"));
        assert_eq!(file.imports()[1].symbols(), vec!["NS".to_string()]);
        assert_eq!(file.imports()[2].symbols(), vec!["All".to_string()]);
    }

    #[test]
    fn empty_file_parses_to_empty_skeleton() {
        let file = parse_ok("");
        assert!(file.pragmas().is_empty());
        assert!(file.imports().is_empty());
        assert!(file.contracts().is_empty());
    }

    #[test]
    fn missing_semicolon_errors_with_span() {
        let text = "pragma solidity ^0.8.0\ncontract C {}";
        let error = SolidityParser.parse_text(text).expect_err("must fail");
        assert!(!error.message.is_empty());
        assert!(error.span.start < text.len() as u32);
    }

    #[test]
    fn contract_without_body_errors() {
        let text = "contract C";
        let error = SolidityParser.parse_text(text).expect_err("must fail");
        assert_eq!(error.message, "expected `{`");
    }

    #[test]
    fn two_broken_contracts_yield_two_errors() {
        let text = "contract ;\ncontract ;\n";
        let lexer = SolidityLexer;
        let tokens = lexer.tokenize(text);
        let (file, errors) = SolidityParser.parse_with_errors(&tokens);
        assert_eq!(errors.len(), 2);
        assert!(file.contracts().is_empty());
        for error in &errors {
            assert_eq!(error.message, "expected contract name");
        }
    }

    #[test]
    fn one_broken_item_keeps_valid_siblings() {
        let text = "contract ;\ncontract Good {}\n";
        let lexer = SolidityLexer;
        let tokens = lexer.tokenize(text);
        let (file, errors) = SolidityParser.parse_with_errors(&tokens);
        assert_eq!(errors.len(), 1);
        assert_eq!(file.contracts().len(), 1);
        assert_eq!(file.contracts()[0].name().as_deref(), Some("Good"));
    }

    #[test]
    fn state_variables_cover_beta_types() {
        use crate::{StateVar, TypeNode};
        let text = "contract C {\nuint256 total;\naddress owner;\nbool flag;\nmapping(address => uint) balances;\nuint[] scores;\nstring memory label;\n}";
        let file = parse_ok(text);
        let contracts = file.contracts();
        assert_eq!(contracts.len(), 1);
        let names: Vec<String> = contracts[0]
            .state_vars()
            .iter()
            .filter_map(StateVar::name)
            .collect();
        assert_eq!(
            names,
            vec!["total", "owner", "flag", "balances", "scores", "label"]
        );
        let types: Vec<String> = contracts[0]
            .state_vars()
            .iter()
            .filter_map(|var| var.type_node().map(|type_node| TypeNode::text(&type_node)))
            .collect();
        assert_eq!(
            types,
            vec![
                "uint256",
                "address",
                "bool",
                "mapping(address => uint)",
                "uint[]",
                "string",
            ]
        );
    }

    #[test]
    fn function_with_params_returns_and_modifiers() {
        let text = "contract C {\nfunction add(uint a, uint b) public pure returns (uint) {\nreturn a;\n}\n}";
        let file = parse_ok(text);
        let functions = file.contracts()[0].functions();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name().as_deref(), Some("add"));
        let params: Vec<String> = functions[0]
            .params()
            .iter()
            .filter_map(|param| param.name())
            .collect();
        assert_eq!(params, vec!["a", "b"]);
        assert_eq!(functions[0].return_types().len(), 1);
        assert!(functions[0].body().is_some());
    }

    #[test]
    fn locals_are_collected_and_statements_skipped() {
        use crate::LocalVar;
        let text =
            "contract C {\nfunction f() public {\nuint x = 1;\nx = x + 1;\nstring memory s;\n}\n}";
        let file = parse_ok(text);
        let body = file.contracts()[0].functions()[0].body().expect("body");
        let names: Vec<String> = body.locals().iter().filter_map(LocalVar::name).collect();
        assert_eq!(names, vec!["x", "s"]);
    }

    #[test]
    fn broken_member_keeps_valid_siblings() {
        let text = "contract C {\nuint256 good;\nuint ;\nbool fine;\n}";
        let lexer = SolidityLexer;
        let tokens = lexer.tokenize(text);
        let (file, errors) = SolidityParser.parse_with_errors(&tokens);
        assert_eq!(errors.len(), 1);
        let names: Vec<String> = file.contracts()[0]
            .state_vars()
            .iter()
            .filter_map(|var| var.name())
            .collect();
        assert_eq!(names, vec!["good", "fine"]);
    }
}
