use super::span_to_range;
use forgels_protocol::{Diagnostic, DiagnosticSeverity};
use forgels_syntax::ParseError;

/// Map a parser error to an editor diagnostic.
///
/// Severity is always error: a file that fails to parse cannot be trusted
/// for hover, completion, or definition. The message passes through
/// untouched so parser wording stays searchable in tests.
pub fn parse_error_to_diagnostic(text: &str, error: &ParseError) -> Diagnostic {
    Diagnostic {
        range: span_to_range(text, error.span),
        severity: Some(DiagnosticSeverity::Error),
        message: error.message.clone(),
    }
}

/// Map every collected error to its own diagnostic, in order.
///
/// Recovery means parsers continue past the first failure and hand over
/// the full list. This mapper never merges or drops: two errors in means
/// two diagnostics out, so each squiggle marks its own span.
pub fn parse_errors_to_diagnostics(text: &str, errors: &[ParseError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| parse_error_to_diagnostic(text, error))
        .collect()
}
