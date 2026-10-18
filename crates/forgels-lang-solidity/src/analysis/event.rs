use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;

use crate::SolidityFile;

fn name_span_or_full(
    name_span: Option<forgels_syntax::Span>,
    full: forgels_syntax::Span,
) -> forgels_syntax::Span {
    name_span.unwrap_or(full)
}

fn all_events(file: &SolidityFile) -> Vec<crate::EventDef> {
    let mut out: Vec<crate::EventDef> = file.events();
    for contract in file.contracts() {
        out.extend(contract.events());
    }
    out
}

/// Event indexed limit for the certain subset.
///
/// Non-anonymous events with more than 3 indexed params error.
/// Anonymous limits, mapping indexed, and string or bytes indexed
/// stay deferred: the spec is unverified, so no check runs for them.
pub fn event_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for event in all_events(file) {
        if event.is_anonymous() {
            continue;
        }
        let indexed = event
            .params()
            .iter()
            .filter(|param| param.is_indexed())
            .count();
        if indexed <= 3 {
            continue;
        }
        let Some(name) = event.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        out.push(DiagnosticData {
            span: name_span_or_full(event.name_span(), event.syntax().span),
            message: format!("event '{name}' has {indexed} indexed params but max is 3"),
        });
    }
    out
}
