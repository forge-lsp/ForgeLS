use crate::{ParseResult, SyntaxNode, Token};

/// Language agnostic parser interface. Each language implements this
/// and returns its own AST rooted in [`SyntaxNode`].
pub trait Parser {
    type Output;

    fn parse(&self, tokens: &[Token]) -> ParseResult<Self::Output>;

    fn parse_text(&self, text: &str) -> ParseResult<Self::Output>
    where
        Self: Sized,
    {
        let _ = (text, std::mem::size_of::<SyntaxNode>());
        Err(crate::ParseError::new(
            crate::Span::new(0, 0),
            format!(
                "parse_text not implemented for {}",
                std::any::type_name::<Self>()
            ),
        ))
    }
}
