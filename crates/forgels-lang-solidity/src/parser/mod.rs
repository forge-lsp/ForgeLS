use crate::SolidityLexer;
use crate::lexer::SolidityKind;
use crate::{SolidityFile, SolidityNode};
use forgels_syntax::{
    Lexer, ParseError, ParseResult, Parser, Span, SyntaxElement, SyntaxNode, Token,
};

mod contract;
mod cursor;
mod items;
mod members;
mod types;

#[cfg(test)]
mod tests;

use contract::parse_contract;
use cursor::Cursor;
use items::{parse_import, parse_pragma};

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
                Some(kind)
                    if kind == SolidityKind::Contract.as_u16()
                        || kind == SolidityKind::Abstract.as_u16() =>
                {
                    match parse_contract(&mut cursor, &mut errors) {
                        Some(node) => items.push(SyntaxElement::Node(node)),
                        None => cursor.recover(),
                    }
                }
                Some(kind) if kind == SolidityKind::Error.as_u16() => {
                    if let Some(token) = cursor.bump() {
                        errors.push(ParseError::new(token.span, "unexpected character"));
                    }
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
