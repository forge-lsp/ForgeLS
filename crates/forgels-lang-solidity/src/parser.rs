use crate::{SolidityFile, SolidityLexer};
use forgels_syntax::{Lexer, ParseResult, Parser, Span, SyntaxNode, Token};

#[derive(Debug, Default)]
pub struct SolidityParser;

impl Parser for SolidityParser {
    type Output = SolidityFile;

    fn parse(&self, tokens: &[Token]) -> ParseResult<Self::Output> {
        let end = tokens.last().map(|t| t.span.end).unwrap_or(0);
        Ok(SolidityFile::new(SyntaxNode::new(0, Span::new(0, end))))
    }

    fn parse_text(&self, text: &str) -> ParseResult<Self::Output> {
        let lexer = SolidityLexer;
        let tokens = lexer.tokenize(text);
        self.parse(&tokens)
    }
}
