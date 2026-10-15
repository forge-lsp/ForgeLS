use crate::Span;

/// Parser failure. Every error carries a span and a message, no exceptions:
/// span locates the squiggle, message explains it. Empty messages are
/// rejected in debug builds because they render as silent markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl ParseError {
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        let message = message.into();
        debug_assert!(!message.is_empty(), "parse error message must not be empty");
        Self { span, message }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {:?}", self.message, self.span)
    }
}

impl std::error::Error for ParseError {}

pub type ParseResult<T> = Result<T, ParseError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_span_and_message() {
        let error = ParseError::new(Span::new(4, 9), "expected type name");
        assert_eq!(error.span, Span::new(4, 9));
        assert_eq!(error.message, "expected type name");
    }

    #[test]
    fn display_shows_message_and_span() {
        let error = ParseError::new(Span::new(4, 9), "expected type name");
        let shown = error.to_string();
        assert!(
            shown.contains("expected type name"),
            "message shown: {shown}"
        );
        assert!(shown.contains("4"), "span shown: {shown}");
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn empty_message_panics_in_debug() {
        let _ = ParseError::new(Span::new(0, 0), "");
    }
}
