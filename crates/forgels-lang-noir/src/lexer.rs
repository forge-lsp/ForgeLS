use forgels_syntax::{Lexer, Span, Token};

#[derive(Debug, Default)]
pub struct NoirLexer;

impl Lexer for NoirLexer {
    fn tokenize(&self, input: &str) -> Vec<Token> {
        // TODO: replace with real Noir lexer.
        vec![Token::new(
            0,
            input.to_string(),
            Span::new(0, input.len() as u32),
        )]
    }
}
