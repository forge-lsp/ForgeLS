use std::collections::HashSet;

use forgels_core::DiagnosticData;
use forgels_syntax::AstNode;

use super::{forge, unknown};
use crate::SolidityFile;

fn is_builtin_word(name: &str) -> bool {
    if unknown::GLOBAL_RECEIVERS.contains(&name) || unknown::ALWAYS_OK.contains(&name) {
        return true;
    }
    forge::ASSERT_TABLE.iter().any(|item| item.name == name)
        || forge::VM_TABLE.iter().any(|item| item.name == name)
        || forge::CONSOLE_TABLE.iter().any(|item| item.name == name)
}

fn push_shadow(
    out: &mut Vec<DiagnosticData>,
    name: &str,
    span: Option<forgels_syntax::Span>,
    is_param: bool,
) {
    let Some(span) = span else {
        return;
    };
    if name.is_empty() {
        return;
    }
    if is_builtin_word(name) {
        return;
    }
    let kind = if is_param { "parameter" } else { "variable" };
    out.push(DiagnosticData {
        span,
        message: format!("{kind} '{name}' shadows outer declaration"),
    });
}

fn walk_block(out: &mut Vec<DiagnosticData>, block: &crate::BlockNode, outer: &HashSet<String>) {
    let mut current: HashSet<String> = HashSet::new();
    for local in block.locals() {
        let Some(name) = local.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let span = local.name_span().or_else(|| Some(local.syntax().span));
        if outer.contains(&name) || current.contains(&name) {
            push_shadow(&mut *out, &name, span, false);
        }
        current.insert(name);
    }
    let mut extended = outer.clone();
    extended.extend(current);
    for child in block.child_blocks() {
        walk_block(&mut *out, &child, &extended);
    }
}

/// Shadowing for locals and params only.
///
/// Inner `Param` or `LocalVar` names that match an outer `StateVar`,
/// `Param`, or ancestor `LocalVar` error. Heritage, functions,
/// contracts, structs, and builtins stay silent: only the three
/// variable kinds participate. Sibling blocks never clash.
pub fn shadow_diagnostics(file: &SolidityFile, _scope: &super::Scope) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        let Some(contract_name) = contract.name() else {
            continue;
        };
        if contract_name.is_empty() {
            continue;
        }
        let mut state_names: HashSet<String> = HashSet::new();
        for var in contract.state_vars() {
            let Some(name) = var.name() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            state_names.insert(name);
        }
        for function in contract.functions() {
            let mut outer = state_names.clone();
            let mut seen_params: HashSet<String> = HashSet::new();
            for param in function.params() {
                let Some(name) = param.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let span = param.name_span().or_else(|| Some(param.syntax().span));
                if outer.contains(&name) || seen_params.contains(&name) {
                    push_shadow(&mut out, &name, span, true);
                }
                seen_params.insert(name.clone());
                outer.insert(name);
            }
            let Some(body) = function.body() else {
                continue;
            };
            walk_block(&mut out, &body, &outer);
        }
    }
    out
}
