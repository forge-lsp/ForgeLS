use crate::{NoirFile, NoirLexer};
use forgels_syntax::{Lexer, ParseResult, Parser, Span, SyntaxNode, Token};

#[derive(Debug, Default)]
pub struct NoirParser;

impl Parser for NoirParser {
    type Output = NoirFile;

    fn parse(&self, tokens: &[Token]) -> ParseResult<Self::Output> {
        let end = tokens.last().map(|t| t.span.end).unwrap_or(0);
        Ok(NoirFile::new(SyntaxNode::new(0, Span::new(0, end))))
    }

    fn parse_text(&self, text: &str) -> ParseResult<Self::Output> {
        let lexer = NoirLexer;
        let tokens = lexer.tokenize(text);
        self.parse(&tokens)
    }
}
