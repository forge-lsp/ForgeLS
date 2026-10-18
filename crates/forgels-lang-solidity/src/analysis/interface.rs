use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;

use crate::SolidityFile;

fn name_span_or_full(
    name_span: Option<forgels_syntax::Span>,
    full: forgels_syntax::Span,
) -> forgels_syntax::Span {
    name_span.unwrap_or(full)
}

/// Interface limits for the certain subset.
///
/// Interfaces cannot declare state variables or constructors.
/// Libraries are fully deferred: constant and immutable need parser
/// support that does not exist yet, so no library check runs here.
pub fn interface_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        if contract.kind_label() != "interface" {
            continue;
        }
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        for var in contract.state_vars() {
            let var_name = var.name().unwrap_or_default();
            if var_name.is_empty() {
                continue;
            }
            out.push(DiagnosticData {
                span: name_span_or_full(var.name_span(), var.syntax().span),
                message: format!(
                    "interface '{contract_name}' cannot have state variable '{var_name}'"
                ),
            });
        }
        for constructor in contract.constructors() {
            out.push(DiagnosticData {
                span: name_span_or_full(constructor.name_span(), constructor.syntax().span),
                message: format!("interface '{contract_name}' cannot have constructor"),
            });
        }
    }
    out
}
