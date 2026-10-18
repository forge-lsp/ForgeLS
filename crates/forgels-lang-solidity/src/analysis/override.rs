use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::AstNode;

use super::inheritance;
use crate::{FunctionDef, SolidityFile, Visibility};

fn is_skipped_head(name: &str) -> bool {
    name == "constructor" || name == "fallback" || name == "receive"
}

fn param_types(function: &FunctionDef) -> Vec<String> {
    function
        .params()
        .iter()
        .map(|param| {
            param
                .type_node()
                .map(|node| node.text())
                .unwrap_or_default()
        })
        .collect()
}

fn same_params(left: &FunctionDef, right: &FunctionDef) -> bool {
    param_types(left) == param_types(right)
}

fn base_candidates(
    file: &SolidityFile,
    contract_name: &str,
    name: &str,
    derived: &FunctionDef,
) -> Vec<FunctionDef> {
    let mut out = Vec::new();
    let mut seen = false;
    for base in inheritance::linearize(file, contract_name) {
        for function in base.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name != name {
                continue;
            }
            if function.visibility() == Some(Visibility::Private) {
                continue;
            }
            if !same_params(&function, derived) {
                continue;
            }
            if seen
                && out.iter().any(|known: &FunctionDef| {
                    known.name().as_deref() == Some(name) && same_params(known, &function)
                })
            {
                continue;
            }
            seen = true;
            out.push(function);
        }
    }
    out
}

fn name_span_or_full(function: &FunctionDef) -> forgels_syntax::Span {
    function
        .name_span()
        .unwrap_or_else(|| function.syntax().span)
}

fn has_any_same_name_base(file: &SolidityFile, contract_name: &str, name: &str) -> bool {
    for base in inheritance::linearize(file, contract_name) {
        for function in base.functions() {
            if function.name().as_deref() == Some(name) {
                return true;
            }
        }
    }
    false
}

/// Override checks O1, O2, O3 with silent overloads.
///
/// O1 errors when a derived function matches a virtual base but misses
/// `override`. O2 errors when `override` has no matching base. O3 errors
/// when `override` matches only non-virtual bases. Silent when the name
/// is a constructor, fallback, or receive, when the derived function is
/// private, when params differ (overload), or when the base is private.
/// State variables never report here. Bases follow C3 `linearize`;
/// multi-base list validation stays deferred.
pub fn override_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        for function in contract.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || is_skipped_head(&func_name) {
                continue;
            }
            if function.visibility() == Some(Visibility::Private) {
                continue;
            }
            let bases = base_candidates(file, &contract_name, &func_name, &function);
            let has_virtual_base = bases.iter().any(|base| base.is_virtual());
            let has_any_base = !bases.is_empty();
            if function.is_override() {
                if !has_any_base {
                    if has_any_same_name_base(file, &contract_name, &func_name) {
                        continue;
                    }
                    out.push(DiagnosticData::error(
                        name_span_or_full(&function),
                        format!("function '{func_name}' has 'override' but no base function"),
                        RuleId::OVERRIDE,
                    ));
                } else if !has_virtual_base {
                    out.push(DiagnosticData::error(
                        name_span_or_full(&function),
                        format!("function '{func_name}' overrides non-virtual base"),
                        RuleId::OVERRIDE,
                    ));
                }
            } else if has_virtual_base {
                out.push(DiagnosticData::error(
                    name_span_or_full(&function),
                    format!("function '{func_name}' overrides virtual base but missing 'override'"),
                    RuleId::OVERRIDE,
                ));
            }
        }
    }
    out
}
