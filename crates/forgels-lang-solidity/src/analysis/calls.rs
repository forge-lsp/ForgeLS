use super::{Scope, SymbolKind};
use crate::SolidityFile;
use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;
use std::collections::HashMap;

/// Arity checks for plain calls: compare declared params with supplied args.
///
/// Unknown names and non-function symbols stay silent. Overloads use
/// the first definition, matching scope resolution order.
pub fn arity_diagnostics(file: &SolidityFile, scope: &Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let mut params_of: HashMap<String, usize> = HashMap::new();
        for function in contract.functions() {
            let Some(name) = function.name() else {
                continue;
            };
            params_of
                .entry(name)
                .or_insert_with(|| function.params().len());
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.calls() {
                let Some(callee) = call.callee() else {
                    continue;
                };
                let span = call.syntax().span;
                let Some(symbol) = scope.resolve(&callee, span.start) else {
                    continue;
                };
                if symbol.kind != SymbolKind::Function {
                    continue;
                }
                let Some(expected) = params_of.get(&callee) else {
                    continue;
                };
                let actual = call.arg_count();
                if *expected != actual {
                    out.push(DiagnosticData {
                        span,
                        message: format!(
                            "this function '{callee}' takes {expected} arguments but {actual} supplied"
                        ),
                    });
                }
            }
        }
    }
    out
}
