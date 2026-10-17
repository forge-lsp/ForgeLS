use std::collections::HashSet;

use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;

use super::inheritance;
use crate::{SolidityFile, Visibility};

fn visibility_of(contract: &crate::ContractDef, name: &str) -> Option<Visibility> {
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name != name {
            continue;
        }
        return function.visibility();
    }
    None
}

/// Lookup `name` in own contract first, then linearized bases.
///
/// Derived wins and the first base wins. Returns the defining function
/// visibility, or `None` when unknown or when the winner has no modifier.
fn lookup_visibility(
    file: &SolidityFile,
    contract: &crate::ContractDef,
    contract_name: &str,
    name: &str,
) -> Option<Visibility> {
    if name.is_empty() {
        return None;
    }
    let mut seen: HashSet<String> = HashSet::new();
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name.is_empty() || !seen.insert(func_name.clone()) {
            continue;
        }
        if func_name == name {
            return visibility_of(contract, name);
        }
    }
    for base in inheritance::linearize(file, contract_name) {
        for function in base.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || !seen.insert(func_name.clone()) {
                continue;
            }
            if func_name == name {
                return function.visibility();
            }
        }
    }
    None
}

/// External functions called as plain calls in the same contract.
///
/// Plain `f()` where `f` is external must go through `this.f()` or
/// `C.f()`. Member calls, `super` calls, and unknown names stay silent.
/// Lookup is own plus linearized bases with derived wins.
pub fn visibility_diagnostics(file: &SolidityFile, scope: &super::Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.calls() {
                let Some(callee) = call.callee() else {
                    continue;
                };
                if callee.is_empty() {
                    continue;
                }
                let offset = call.syntax().span.start;
                // Member and super forms never reach here as Call nodes,
                // but guard the scope kind anyway: non-functions stay silent.
                if let Some(symbol) = scope.resolve(&callee, offset)
                    && symbol.kind != super::SymbolKind::Function
                {
                    continue;
                }
                let Some(vis) = lookup_visibility(file, &contract, &contract_name, &callee) else {
                    continue;
                };
                if vis != Visibility::External {
                    continue;
                }
                out.push(DiagnosticData {
                    span: call.syntax().span,
                    message: format!(
                        "external function '{callee}' must be called via 'this.{callee}()' or '{contract_name}.{callee}()'"
                    ),
                });
            }
        }
    }
    out
}
