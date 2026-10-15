use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: u16,
    pub text: String,
    pub span: Span,
}

impl Token {
    pub fn new(kind: u16, text: impl Into<String>, span: Span) -> Self {
        Self {
            kind,
            text: text.into(),
            span,
        }
    }
}

/// Language agnostic lexer interface. Each language implements this.
pub trait Lexer {
    fn tokenize(&self, input: &str) -> Vec<Token>;
}
