use crate::lexer::SolidityKind;
use forgels_syntax::{ParseError, Span, SyntaxElement, Token};

/// Token cursor that hides comments: the parser never sees trivia.
pub(crate) struct Cursor<'a> {
    pub(crate) tokens: &'a [Token],
    pos: usize,
    eof: Span,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(tokens: &'a [Token], file_end: u32) -> Self {
        Self {
            tokens,
            pos: 0,
            eof: Span::new(file_end, file_end),
        }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn at_end(&mut self) -> bool {
        self.peek().is_none()
    }

    pub(crate) fn peek(&mut self) -> Option<&'a Token> {
        while let Some(token) = self.tokens.get(self.pos) {
            if token.kind == crate::lexer::SolidityKind::Comment.as_u16() {
                self.pos += 1;
            } else {
                return Some(token);
            }
        }
        None
    }

    pub(crate) fn peek_kind(&mut self) -> Option<u16> {
        self.peek().map(|token| token.kind)
    }

    pub(crate) fn peek_span(&mut self) -> Span {
        self.peek().map(|token| token.span).unwrap_or(self.eof)
    }

    pub(crate) fn bump(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        self.pos += 1;
        Some(token)
    }

    pub(crate) fn expect(&mut self, kind: SolidityKind, what: &str) -> Result<Token, ParseError> {
        match self.peek_kind() {
            Some(found) if found == kind.as_u16() => self.bump_or_error(what),
            _ => Err(ParseError::new(
                self.peek_span(),
                format!("expected {what}"),
            )),
        }
    }

    /// Bump the peeked token, or a spanned error when nothing is left.
    /// Use this instead of `expect` on `bump`: peeking and bumping are
    /// separate steps, so only this form is panic-free by construction.
    pub(crate) fn bump_or_error(&mut self, what: &str) -> Result<Token, ParseError> {
        self.bump()
            .ok_or_else(|| ParseError::new(self.peek_span(), format!("expected {what}")))
    }

    /// Skip to the next sync point: past `;`, or stop before a top-level
    /// keyword, `}`, or EOF. Always moves forward.
    pub(crate) fn recover(&mut self) {
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
                    || k == SolidityKind::Abstract.as_u16()
                    || k == SolidityKind::Struct.as_u16()
                    || k == SolidityKind::Enum.as_u16()
                    || k == SolidityKind::ErrorKw.as_u16()
                    || k == SolidityKind::Type.as_u16()
                    || k == SolidityKind::RBrace.as_u16()
            ) {
                return;
            }
            self.bump();
        }
    }
}

pub(crate) fn token_element(token: Token) -> SyntaxElement {
    SyntaxElement::Token(forgels_syntax::SyntaxToken {
        kind: token.kind,
        text: token.text,
        span: token.span,
    })
}
