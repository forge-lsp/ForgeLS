use super::{Scope, SymbolKind, inheritance};
use crate::SolidityFile;
use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;
use std::collections::HashMap;

/// Arity checks for plain calls: compare declared params with supplied args.
///
/// Inherited functions count via heritage order linearization, derived
/// wins and first base wins. Unknown names and non-function symbols stay
/// silent. Overloads use the first definition, matching scope order.
pub fn arity_diagnostics(file: &SolidityFile, scope: &Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        let mut params_of: HashMap<String, usize> = HashMap::new();
        for function in contract.functions() {
            let Some(name) = function.name() else {
                continue;
            };
            params_of
                .entry(name)
                .or_insert_with(|| function.params().len());
        }
        for base in inheritance::linearize(file, &contract_name) {
            for function in base.functions() {
                let Some(name) = function.name() else {
                    continue;
                };
                params_of
                    .entry(name)
                    .or_insert_with(|| function.params().len());
            }
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
