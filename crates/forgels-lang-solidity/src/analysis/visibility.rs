use std::collections::HashSet;
use std::path::PathBuf;

use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::AstNode;

use super::{inheritance, members};
use crate::{SolidityFile, Visibility};

/// Single-contract visibility with overload silence.
///
/// Gathers every function named `name` in `contract`. Mixed
/// visibilities or missing modifiers give `None` so overloads with
/// different visibility never lint. Single uniform hit returns it.
fn visibility_in(contract: &crate::ContractDef, name: &str) -> Option<Visibility> {
    if name.is_empty() {
        return None;
    }
    let mut found: Vec<Option<Visibility>> = Vec::new();
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name != name {
            continue;
        }
        found.push(function.visibility());
    }
    if found.is_empty() {
        return None;
    }
    let first = found[0]?;
    for item in &found[1..] {
        if *item != Some(first) {
            return None;
        }
    }
    Some(first)
}

/// Lookup `name` in own contract first, then linearized bases.
///
/// Derived wins and the first base wins. Same-file bases come first,
/// then dep bases with dedupe by contract name. Returns the winning
/// visibility plus true when defined in the caller itself. Unknown,
/// missing modifiers, and mixed overloads give `None`.
fn lookup_visibility(
    file: &SolidityFile,
    contract: &crate::ContractDef,
    contract_name: &str,
    name: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(Visibility, bool)> {
    if name.is_empty() {
        return None;
    }
    if let Some(vis) = visibility_in(contract, name) {
        return Some((vis, true));
    }
    if has_mixed_overload(contract, name) {
        return None;
    }
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(contract_name.to_string());
    for base in inheritance::linearize(file, contract_name) {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() || !seen.insert(base_name.clone()) {
            continue;
        }
        if has_mixed_overload(&base, name) {
            return None;
        }
        if let Some(vis) = visibility_in(&base, name) {
            return Some((vis, false));
        }
    }
    for (base, _) in inheritance::linearize_across(file, contract_name, deps) {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() || !seen.insert(base_name.clone()) {
            continue;
        }
        if has_mixed_overload(&base, name) {
            return None;
        }
        if let Some(vis) = visibility_in(&base, name) {
            return Some((vis, false));
        }
    }
    None
}

fn has_mixed_overload(contract: &crate::ContractDef, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut seen: Vec<Option<Visibility>> = Vec::new();
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name != name {
            continue;
        }
        seen.push(function.visibility());
    }
    if seen.len() <= 1 {
        return false;
    }
    let first = seen[0];
    for item in &seen[1..] {
        if *item != first {
            return true;
        }
    }
    false
}

/// Visibility of `member` in `target` plus its bases.
///
/// Own target first, then same-file plus dep bases in linear order.
/// Mixed overloads give `None` so differing visibility stays silent.
fn target_visibility(
    file: &SolidityFile,
    target: &str,
    member: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Visibility> {
    if target.is_empty() || member.is_empty() {
        return None;
    }
    let (def, _) = inheritance::find_contract_across(target, file, deps)?;
    if has_mixed_overload(&def, member) {
        return None;
    }
    if let Some(vis) = visibility_in(&def, member) {
        return Some(vis);
    }
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(name) = def.name() {
        seen.insert(name);
    }
    for base in inheritance::linearize(file, target) {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() || !seen.insert(base_name.clone()) {
            continue;
        }
        if has_mixed_overload(&base, member) {
            return None;
        }
        if let Some(vis) = visibility_in(&base, member) {
            return Some(vis);
        }
    }
    for (base, _) in inheritance::linearize_across(file, target, deps) {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() || !seen.insert(base_name.clone()) {
            continue;
        }
        if has_mixed_overload(&base, member) {
            return None;
        }
        if let Some(vis) = visibility_in(&base, member) {
            return Some(vis);
        }
    }
    None
}

fn is_library(file: &SolidityFile, target: &str, deps: &[(PathBuf, SolidityFile)]) -> bool {
    if target.is_empty() {
        return false;
    }
    let Some((def, _)) = inheritance::find_contract_across(target, file, deps) else {
        return false;
    };
    def.kind_label() == "library"
}

/// Visibility checks: external plain calls, private plain calls in
/// derived contracts, and instance internal or private calls.
///
/// Plain `f()` where `f` is external must go through `this.f()` or
/// `C.f()`. Plain `f()` where `f` is private in a base errors in the
/// derived contract. Instance `c.f()` where `f` is internal or private
/// in the receiver contract errors. `this`, `super`, library targets,
/// internal calls through inheritance, and mixed overloads stay silent.
/// Lookup is own plus linearized bases same file and across deps with
/// derived wins.
pub fn visibility_diagnostics(
    file: &SolidityFile,
    scope: &super::Scope,
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
                let Some((vis, is_own)) =
                    lookup_visibility(file, &contract, &contract_name, &callee, deps)
                else {
                    continue;
                };
                if vis == Visibility::External {
                    out.push(DiagnosticData::error(
                        call.syntax().span,
                        format!(
                            "external function '{callee}' must be called via 'this.{callee}()' or '{contract_name}.{callee}()'"
                        ),
                        RuleId::VISIBILITY,
                    ));
                    continue;
                }
                if vis == Visibility::Private && !is_own {
                    out.push(DiagnosticData::error(
                        call.syntax().span,
                        format!(
                            "private function '{callee}' cannot be called from derived contract '{contract_name}'"
                        ),
                        RuleId::VISIBILITY,
                    ));
                }
            }
            for member in body.all_members() {
                let chain = member.chain();
                if chain.len() != 2 {
                    continue;
                }
                let root = chain.first().cloned().unwrap_or_default();
                if root.is_empty() || root == "this" || root == "super" {
                    continue;
                }
                let Some(name) = member.final_member() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let offset = member.syntax().span.start;
                let kind = members::receiver_type_with_deps(file, scope, offset, &root, deps);
                let members::TypeKind::Contract(target) = kind else {
                    continue;
                };
                if target.is_empty() {
                    continue;
                }
                if is_library(file, &target, deps) {
                    continue;
                }
                let Some(vis) = target_visibility(file, &target, &name, deps) else {
                    continue;
                };
                if vis == Visibility::Internal {
                    out.push(DiagnosticData::error(
                        member.syntax().span,
                        format!(
                            "internal function '{name}' cannot be called via instance '{root}'"
                        ),
                        RuleId::VISIBILITY,
                    ));
                    continue;
                }
                if vis == Visibility::Private {
                    out.push(DiagnosticData::error(
                        member.syntax().span,
                        format!("private function '{name}' cannot be called via instance '{root}'"),
                        RuleId::VISIBILITY,
                    ));
                }
            }
        }
    }
    out
}
