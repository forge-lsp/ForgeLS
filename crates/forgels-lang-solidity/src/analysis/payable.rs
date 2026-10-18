use std::collections::HashSet;
use std::path::PathBuf;

use forgels_core::DiagnosticData;
use forgels_syntax::{AstNode, Lexer};

use super::{Scope, SymbolKind, forge, inheritance, members};
use crate::{SolidityFile, SolidityLexer, lexer::SolidityKind};

fn options_contain_value(text: &str, span: forgels_syntax::Span) -> bool {
    let tokens = SolidityLexer.tokenize(text);
    for token in &tokens {
        if token.kind != SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text != "value" {
            continue;
        }
        if span.contains(token.span.start) {
            return true;
        }
    }
    false
}

fn own_function(
    contract: &crate::ContractDef,
    file: &SolidityFile,
    contract_name: &str,
    name: &str,
) -> Option<crate::FunctionDef> {
    let mut seen: HashSet<String> = HashSet::new();
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name.is_empty() || !seen.insert(func_name.clone()) {
            continue;
        }
        if func_name == name {
            return Some(function);
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
                return Some(function);
            }
        }
    }
    None
}

fn target_function(
    file: &SolidityFile,
    target: &str,
    name: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<crate::FunctionDef> {
    if target.is_empty() || name.is_empty() {
        return None;
    }
    let (def, _) = inheritance::find_contract_across(target, file, deps)?;
    let mut seen: HashSet<String> = HashSet::new();
    for function in def.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name.is_empty() || !seen.insert(func_name.clone()) {
            continue;
        }
        if func_name == name {
            return Some(function);
        }
    }
    for base in inheritance::linearize(file, target) {
        for function in base.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || !seen.insert(func_name.clone()) {
                continue;
            }
            if func_name == name {
                return Some(function);
            }
        }
    }
    for (base, _) in inheritance::linearize_across(file, target, deps) {
        for function in base.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || !seen.insert(func_name.clone()) {
                continue;
            }
            if func_name == name {
                return Some(function);
            }
        }
    }
    None
}

/// Base function for `super` member checks: same file bases plus deps.
///
/// Derived functions never count: `super` bypasses the enclosing
/// contract. First base wins on name clash, same as member lookup.
fn super_function(
    file: &SolidityFile,
    enclosing: &str,
    name: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<crate::FunctionDef> {
    if enclosing.is_empty() || name.is_empty() {
        return None;
    }
    let mut seen: HashSet<String> = HashSet::new();
    for base in inheritance::linearize(file, enclosing) {
        for function in base.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || !seen.insert(func_name.clone()) {
                continue;
            }
            if func_name == name {
                return Some(function);
            }
        }
    }
    for (base, _) in inheritance::linearize_across(file, enclosing, deps) {
        for function in base.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || !seen.insert(func_name.clone()) {
                continue;
            }
            if func_name == name {
                return Some(function);
            }
        }
    }
    None
}

/// Non-payable functions must not receive `value` call options.
///
/// Plain `f{value: 1}()` and member `o.f{value: 1}()` error when the
/// target resolves to a function without `payable`. `super.f{value: 1}`
/// resolves through the base lineage across files. Builtins, unknown
/// targets, and gas-only options stay silent.
pub fn payable_diagnostics(
    file: &SolidityFile,
    text: &str,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<DiagnosticData> {
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
            for call in body.all_calls() {
                if !call.has_call_options() {
                    continue;
                }
                let Some(options) = call.options_span() else {
                    continue;
                };
                if !options_contain_value(text, options) {
                    continue;
                }
                let Some(callee) = call.callee() else {
                    continue;
                };
                if callee.is_empty() {
                    continue;
                }
                let offset = call.syntax().span.start;
                if let Some(symbol) = scope.resolve(&callee, offset)
                    && symbol.kind != SymbolKind::Function
                {
                    continue;
                }
                let Some(target) = own_function(&contract, file, &contract_name, &callee) else {
                    continue;
                };
                if target.mutability() == Some(crate::Mutability::Payable) {
                    continue;
                }
                out.push(DiagnosticData::error(
                    call.syntax().span,
                    format!("non-payable function '{callee}' cannot receive 'value'"),
                ));
            }
            for member in body.all_members() {
                if !member.has_call_options() {
                    continue;
                }
                if !member.is_call() {
                    continue;
                }
                let Some(options) = member.options_span() else {
                    continue;
                };
                if !options_contain_value(text, options) {
                    continue;
                }
                let chain = member.chain();
                if chain.len() < 2 {
                    continue;
                }
                let Some(name) = member.final_member() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let offset = member.syntax().span.start;
                if chain.len() == 2 {
                    let root = chain[0].clone();
                    if root.is_empty() {
                        continue;
                    }
                    if forge::member_builtin(&root, &name, scope, offset).is_some() {
                        continue;
                    }
                }
                let kind = members::resolve_chained_kind(file, scope, offset, &chain, deps);
                let found = match kind {
                    members::TypeKind::Super { enclosing } => {
                        super_function(file, &enclosing, &name, deps)
                    }
                    members::TypeKind::Contract(target) => {
                        target_function(file, &target, &name, deps)
                    }
                    _ => None,
                };
                let Some(found) = found else {
                    continue;
                };
                if found.mutability() == Some(crate::Mutability::Payable) {
                    continue;
                }
                out.push(DiagnosticData::error(
                    member.syntax().span,
                    format!("non-payable function '{name}' cannot receive 'value'"),
                ));
            }
        }
    }
    out
}
