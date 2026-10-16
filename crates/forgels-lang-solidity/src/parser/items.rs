use super::cursor::{Cursor, token_element};
use crate::SolidityNode;
use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, SyntaxNode, Token};

pub(crate) fn parse_pragma(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start = cursor.bump_or_error("pragma keyword")?.span;
    let name = cursor.expect(SolidityKind::Ident, "pragma name")?;
    let mut children = vec![token_element(name)];
    let mut saw_version = false;
    while let Some(kind) = cursor.peek_kind() {
        if kind == SolidityKind::Semicolon.as_u16()
            || kind == SolidityKind::Pragma.as_u16()
            || kind == SolidityKind::Import.as_u16()
            || kind == SolidityKind::Contract.as_u16()
            || kind == SolidityKind::Abstract.as_u16()
            || kind == SolidityKind::Struct.as_u16()
            || kind == SolidityKind::Enum.as_u16()
            || kind == SolidityKind::RBrace.as_u16()
        {
            break;
        }
        let token = cursor.bump_or_error("pragma token")?;
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

pub(crate) fn parse_import(cursor: &mut Cursor) -> Result<SyntaxNode, ParseError> {
    let start = cursor.bump_or_error("import keyword")?.span;
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
