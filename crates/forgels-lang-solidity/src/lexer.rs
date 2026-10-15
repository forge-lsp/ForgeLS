use forgels_syntax::{Lexer, Span, Token};

#[derive(Debug, Default)]
pub struct SolidityLexer;

impl Lexer for SolidityLexer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        // TODO: replace with real Solidity lexer.
        vec![Token::new(
            0,
            input.to_string(),
            Span::new(0, input.len() as u32),
        )]
    }
}
