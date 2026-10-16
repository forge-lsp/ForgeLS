use std::collections::HashSet;
use std::path::PathBuf;

use forgels_syntax::AstNode;

use crate::{ContractDef, SolidityFile, StructDef};

use super::members::MemberInfo;
use super::{Symbol, SymbolKind};

/// Find a contract by name in one file.
pub(crate) fn find_contract(file: &SolidityFile, name: &str) -> Option<ContractDef> {
    file.contracts()
        .into_iter()
        .find(|contract| contract.name().as_deref() == Some(name))
}

/// Find a struct by name: file level first, then inside contracts.
pub(crate) fn find_struct(file: &SolidityFile, name: &str) -> Option<StructDef> {
    for item in file.structs() {
        if item.name().as_deref() == Some(name) {
            return Some(item);
        }
    }
    for contract in file.contracts() {
        for item in contract.structs() {
            if item.name().as_deref() == Some(name) {
                return Some(item);
            }
        }
    }
    None
}

/// Linearize same file bases in heritage order with DFS.
///
/// Heritage order wins, transitive bases come before direct ones, and
/// `visited` dedupes diamonds and breaks cycles. This is not C3: V1 keeps
/// declaration order only.
pub(crate) fn linearize(file: &SolidityFile, name: &str) -> Vec<ContractDef> {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(name.to_string());
    let mut out: Vec<ContractDef> = Vec::new();
    walk_same(file, name, &mut visited, &mut out);
    out
}

fn walk_same(
    file: &SolidityFile,
    name: &str,
    visited: &mut HashSet<String>,
    out: &mut Vec<ContractDef>,
) {
    let Some(contract) = find_contract(file, name) else {
        return;
    };
    for base in contract.heritage() {
        if !visited.insert(base.clone()) {
            continue;
        }
        walk_same(file, &base, visited, out);
        if let Some(def) = find_contract(file, &base) {
            out.push(def);
        }
    }
}

fn function_detail(name: &str, params: Vec<String>, returns: Vec<String>) -> String {
    let mut detail = format!("function {name}({})", params.join(", "));
    if !returns.is_empty() {
        detail.push_str(" returns (");
        detail.push_str(&returns.join(", "));
        detail.push(')');
    }
    detail
}

fn function_parts(contract: &ContractDef, index: usize) -> Option<(String, String, SymbolKind)> {
    let functions = contract.functions();
    let function = functions.get(index)?;
    let name = function.name()?;
    if name.is_empty() {
        return None;
    }
    let params: Vec<String> = function
        .params()
        .iter()
        .map(|param| {
            let mut text = param
                .type_node()
                .map(|node| node.text())
                .unwrap_or_default();
            if let Some(param_name) = param.name() {
                text.push(' ');
                text.push_str(&param_name);
            }
            text
        })
        .collect();
    let returns: Vec<String> = function
        .return_types()
        .iter()
        .map(|node| node.text())
        .collect();
    Some((
        name.clone(),
        function_detail(&name, params, returns),
        SymbolKind::Function,
    ))
}

/// Same file inherited symbols with ` - from Base` origin.
///
/// Deduped among bases in linear order: first base wins. Caller filters
/// own names so the derived contract wins.
pub(crate) fn inherited_symbols(file: &SolidityFile, contract: &ContractDef) -> Vec<Symbol> {
    let Some(name) = contract.name() else {
        return Vec::new();
    };
    if name.is_empty() {
        return Vec::new();
    }
    let chain = linearize(file, &name);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<Symbol> = Vec::new();
    for base in &chain {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() {
            continue;
        }
        let suffix = format!(" - from {base_name}");
        for item in base.structs() {
            let Some(item_name) = item.name() else {
                continue;
            };
            if item_name.is_empty() || !seen.insert(item_name.clone()) {
                continue;
            }
            let mut detail = String::from("struct ");
            detail.push_str(&item_name);
            detail.push_str(&suffix);
            out.push(Symbol {
                name: item_name,
                kind: SymbolKind::Struct,
                span: item.syntax().span,
                detail,
            });
        }
        for item in base.enums() {
            let Some(item_name) = item.name() else {
                continue;
            };
            if item_name.is_empty() || !seen.insert(item_name.clone()) {
                continue;
            }
            let mut detail = String::from("enum ");
            detail.push_str(&item_name);
            detail.push_str(&suffix);
            out.push(Symbol {
                name: item_name,
                kind: SymbolKind::Enum,
                span: item.syntax().span,
                detail,
            });
        }
        for item in base.events() {
            let Some(item_name) = item.name() else {
                continue;
            };
            if item_name.is_empty() || !seen.insert(item_name.clone()) {
                continue;
            }
            let mut detail = String::from("event ");
            detail.push_str(&item_name);
            detail.push_str(&suffix);
            out.push(Symbol {
                name: item_name,
                kind: SymbolKind::Event,
                span: item.syntax().span,
                detail,
            });
        }
        for item in base.modifiers() {
            let Some(item_name) = item.name() else {
                continue;
            };
            if item_name.is_empty() || !seen.insert(item_name.clone()) {
                continue;
            }
            let mut detail = String::from("modifier ");
            detail.push_str(&item_name);
            detail.push_str(&suffix);
            out.push(Symbol {
                name: item_name,
                kind: SymbolKind::Modifier,
                span: item.syntax().span,
                detail,
            });
        }
        for var in base.state_vars() {
            let Some(var_name) = var.name() else {
                continue;
            };
            if var_name.is_empty() || !seen.insert(var_name.clone()) {
                continue;
            }
            let mut detail = var.type_node().map(|node| node.text()).unwrap_or_default();
            detail.push(' ');
            detail.push_str(&var_name);
            detail.push_str(&suffix);
            out.push(Symbol {
                name: var_name,
                kind: SymbolKind::StateVar,
                span: var.syntax().span,
                detail,
            });
        }
        for index in 0..base.functions().len() {
            let Some((func_name, func_detail, kind)) = function_parts(base, index) else {
                continue;
            };
            if !seen.insert(func_name.clone()) {
                continue;
            }
            let Some(func) = base.functions().get(index).cloned() else {
                continue;
            };
            let mut detail = func_detail;
            detail.push_str(&suffix);
            out.push(Symbol {
                name: func_name,
                kind,
                span: func.syntax().span,
                detail,
            });
        }
    }
    out
}

/// Same file inherited members with ` - from Base` origin.
///
/// Covers state vars plus functions. Deduped in linear order, caller
/// filters own names so derived wins.
pub(crate) fn inherited_members(file: &SolidityFile, contract_name: &str) -> Vec<MemberInfo> {
    if contract_name.is_empty() {
        return Vec::new();
    }
    let chain = linearize(file, contract_name);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<MemberInfo> = Vec::new();
    for base in &chain {
        let Some(base_name) = base.name() else {
            continue;
        };
        if base_name.is_empty() {
            continue;
        }
        let suffix = format!(" - from {base_name}");
        for var in base.state_vars() {
            let Some(var_name) = var.name() else {
                continue;
            };
            if var_name.is_empty() || !seen.insert(var_name.clone()) {
                continue;
            }
            let mut detail = var.type_node().map(|node| node.text()).unwrap_or_default();
            detail.push(' ');
            detail.push_str(&var_name);
            detail.push_str(&suffix);
            out.push(MemberInfo {
                name: var_name,
                detail,
                kind: SymbolKind::StateVar,
                span: var.syntax().span,
            });
        }
        for index in 0..base.functions().len() {
            let Some((func_name, mut func_detail, kind)) = function_parts(base, index) else {
                continue;
            };
            if !seen.insert(func_name.clone()) {
                continue;
            }
            let Some(func) = base.functions().get(index).cloned() else {
                continue;
            };
            func_detail.push_str(&suffix);
            out.push(MemberInfo {
                name: func_name,
                detail: func_detail,
                kind,
                span: func.syntax().span,
            });
        }
    }
    out
}

/// Find a contract in the primary file first, then deps in order.
pub(crate) fn find_contract_across(
    name: &str,
    primary: &SolidityFile,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(ContractDef, Option<PathBuf>)> {
    if let Some(found) = find_contract(primary, name) {
        return Some((found, None));
    }
    for (path, file) in deps {
        if let Some(found) = find_contract(file, name) {
            return Some((found, Some(path.clone())));
        }
    }
    None
}

/// Find a struct in the primary file first, then deps in order.
pub(crate) fn find_struct_across(
    name: &str,
    primary: &SolidityFile,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(StructDef, Option<PathBuf>)> {
    if let Some(found) = find_struct(primary, name) {
        return Some((found, None));
    }
    for (path, file) in deps {
        if let Some(found) = find_struct(file, name) {
            return Some((found, Some(path.clone())));
        }
    }
    None
}

/// Linearize bases across the primary file plus deps.
///
/// Uses the same heritage order DFS as same file linearization, but each
/// base lookup spans primary then deps. `visited` is by contract name, so
/// diamonds collapse and cycles terminate. Returns each base with its
/// owning path (`None` for primary).
pub(crate) fn linearize_across(
    primary: &SolidityFile,
    name: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<(ContractDef, Option<PathBuf>)> {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(name.to_string());
    let mut out: Vec<(ContractDef, Option<PathBuf>)> = Vec::new();
    walk_across(primary, name, deps, &mut visited, &mut out);
    out
}

fn walk_across(
    primary: &SolidityFile,
    name: &str,
    deps: &[(PathBuf, SolidityFile)],
    visited: &mut HashSet<String>,
    out: &mut Vec<(ContractDef, Option<PathBuf>)>,
) {
    let Some((contract, _)) = find_contract_across(name, primary, deps) else {
        return;
    };
    for base in contract.heritage() {
        if base.is_empty() || !visited.insert(base.clone()) {
            continue;
        }
        walk_across(primary, &base, deps, visited, out);
        if let Some((def, path)) = find_contract_across(&base, primary, deps) {
            out.push((def, path));
        }
    }
}
