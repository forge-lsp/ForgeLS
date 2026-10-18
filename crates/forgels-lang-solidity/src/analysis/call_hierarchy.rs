use std::collections::HashMap;
use std::path::{Path, PathBuf};

use forgels_core::{
    CallHierarchyCallData, CallHierarchyItemData, MAX_CALL_HIERARCHY, ProjectContext,
};
use forgels_syntax::{AstNode, Lexer, Span};

use super::{collect_symbols, word_at};
use crate::SolidityFile;

const CAP: usize = MAX_CALL_HIERARCHY;

fn function_detail(function: &crate::FunctionDef) -> String {
    let name = function.name().unwrap_or_default();
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
    let mut detail = format!("function {name}({})", params.join(", "));
    let returns: Vec<String> = function
        .return_types()
        .iter()
        .map(|node| node.text())
        .collect();
    if !returns.is_empty() {
        detail.push_str(" returns (");
        detail.push_str(&returns.join(", "));
        detail.push(')');
    }
    detail
}

fn function_item(path: PathBuf, function: &crate::FunctionDef) -> Option<CallHierarchyItemData> {
    let name = function.name()?;
    if name.is_empty() {
        return None;
    }
    let full = function.syntax().span;
    let selection = function.name_span().unwrap_or(full);
    Some(CallHierarchyItemData::new(
        name,
        forgels_protocol::SymbolKind::METHOD,
        function_detail(function),
        path,
        full,
        selection,
    ))
}

fn contract_at(file: &SolidityFile, offset: u32) -> Option<crate::ContractDef> {
    for contract in file.contracts() {
        let span = contract.syntax().span;
        if span.contains(offset) || span.end == offset {
            return Some(contract);
        }
    }
    None
}

fn push_plain_functions(
    contract: &crate::ContractDef,
    name: &str,
    out: &mut Vec<crate::FunctionDef>,
) {
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name != name {
            continue;
        }
        if out
            .iter()
            .any(|known| known.syntax().span == function.syntax().span)
        {
            continue;
        }
        out.push(function);
    }
}

fn is_guarded_builtin(file: &SolidityFile, text: &str, offset: u32, scope: &super::Scope) -> bool {
    super::forge::is_builtin_at(file, text, offset, scope)
        || super::array::is_array_at(file, text, offset, scope)
        || super::address::is_address_at(file, text, offset, scope)
        || super::bytes::is_bytes_at(file, text, offset, scope)
        || super::bytes::is_string_at(file, text, offset, scope)
        || super::function::is_function_at(file, text, offset, scope)
        || super::globals::is_global_at(file, text, offset, scope)
        || super::meta::is_meta_at(text, offset)
}

fn resolve_plain_function(
    file: &SolidityFile,
    scope: &super::Scope,
    deps: &[(PathBuf, SolidityFile)],
    anchor: u32,
    name: &str,
    current: &Path,
) -> Option<(PathBuf, crate::FunctionDef)> {
    if name.is_empty() {
        return None;
    }
    let Some(symbol) = scope.resolve(name, anchor) else {
        for (path, dep_file) in deps {
            let dep_scope = collect_symbols(dep_file);
            if dep_scope
                .export_symbols()
                .iter()
                .any(|item| item.name == name)
            {
                for contract in dep_file.contracts() {
                    let mut out = Vec::new();
                    push_plain_functions(&contract, name, &mut out);
                    if let Some(first) = out.into_iter().next() {
                        return Some((path.clone(), first));
                    }
                }
            }
        }
        return None;
    };
    if symbol.kind != super::SymbolKind::Function {
        return None;
    }
    let contract = contract_at(file, anchor)?;
    let mut out = Vec::new();
    push_plain_functions(&contract, name, &mut out);
    if let Some(contract_name) = contract.name()
        && !contract_name.is_empty()
    {
        for base in super::inheritance::linearize(file, &contract_name) {
            push_plain_functions(&base, name, &mut out);
        }
    }
    out.into_iter()
        .next()
        .map(|found| (current.to_path_buf(), found))
}

fn resolve_target_function(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(PathBuf, crate::FunctionDef)> {
    let scope = collect_symbols(file);
    if let Some(member) = super::members::member_context_at(text, offset)
        && (member.member_span.contains(offset) || member.member_span.end == offset)
        && !member.member.is_empty()
    {
        if is_guarded_builtin(file, text, offset, &scope) {
            return None;
        }
        let kind =
            super::members::parent_kind_for_member(file, &scope, text, offset, &member, deps);
        if !matches!(
            kind,
            super::members::TypeKind::Contract(_) | super::members::TypeKind::Struct(_)
        ) {
            return None;
        }
        if matches!(kind, super::members::TypeKind::Struct(_)) {
            return None;
        }
        let super::members::TypeKind::Contract(target) = kind else {
            return None;
        };
        let (def, dep_path) = super::inheritance::find_contract_across(&target, file, deps)?;
        let mut defs = Vec::new();
        push_plain_functions(&def, &member.member, &mut defs);
        for base in super::inheritance::linearize(file, &target) {
            push_plain_functions(&base, &member.member, &mut defs);
        }
        for (base, _) in super::inheritance::linearize_across(file, &target, deps) {
            push_plain_functions(&base, &member.member, &mut defs);
        }
        let found = defs.into_iter().next()?;
        let path = dep_path.unwrap_or_else(|| current.to_path_buf());
        return Some((path, found));
    }
    if super::emits::emit_name_at(text, offset).is_some() {
        return None;
    }
    let (word, _) = word_at(text, offset)?;
    if word.is_empty() {
        return None;
    }
    if is_guarded_builtin(file, text, offset, &scope) {
        return None;
    }
    resolve_plain_function(file, &scope, deps, offset, &word, current)
}

fn enclosing_function(file: &SolidityFile, offset: u32) -> Option<crate::FunctionDef> {
    for contract in file.contracts() {
        for function in contract.functions() {
            let span = function.syntax().span;
            if span.contains(offset) || span.end == offset {
                return Some(function);
            }
        }
    }
    None
}

fn parse_text(text: &str) -> SolidityFile {
    let tokens = crate::SolidityLexer.tokenize(text);
    let (file, _) = crate::SolidityParser.parse_with_errors(&tokens);
    file
}

/// Prepare result for the function at `offset`.
///
/// Only functions resolve: members need a contract receiver, plain names
/// need a function scope entry, emits and builtins stay silent. Identity
/// matches `find_references` span checks so callers and callees line up.
pub fn prepare_call_in(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<CallHierarchyItemData> {
    let (path, function) = resolve_target_function(file, text, offset, current, deps)?;
    function_item(path, &function)
}

/// Callers of the function at `offset`, grouped by caller.
///
/// Reuses `find_references` for usage identity, then maps each usage to
/// its enclosing function. Local fast path stays single file, other
/// names scan tracked files path-sorted. Caps callers at
/// `MAX_CALL_HIERARCHY`.
pub fn incoming_in(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
    ctx: &ProjectContext,
) -> Option<Vec<CallHierarchyCallData>> {
    let _ = resolve_target_function(file, text, offset, current, deps)?;
    let refs = super::references::find_references(file, text, offset, current, deps, ctx)?;
    if refs.is_empty() {
        return Some(Vec::new());
    }
    let mut grouped: HashMap<(PathBuf, u32, u32), (CallHierarchyItemData, Vec<Span>)> =
        HashMap::new();
    for entry in refs.iter().filter(|item| !item.is_declaration) {
        if grouped.len() >= CAP && !grouped.keys().any(|(path, _, _)| path == &entry.path) {
            continue;
        }
        let file_text: String = if entry.path.as_path() == current {
            text.to_string()
        } else {
            let Some(cached) = ctx.file_text(&entry.path) else {
                continue;
            };
            cached.to_string()
        };
        if file_text
            .get(entry.span.start as usize..entry.span.end as usize)
            .is_none()
        {
            continue;
        }
        let candidate = parse_text(&file_text);
        let Some(caller) = enclosing_function(&candidate, entry.span.start) else {
            continue;
        };
        let Some(item) = function_item(entry.path.clone(), &caller) else {
            continue;
        };
        let key = (item.path.clone(), item.span.start, item.span.end);
        grouped
            .entry(key)
            .or_insert_with(|| (item, Vec::new()))
            .1
            .push(entry.span);
    }
    let mut out: Vec<CallHierarchyCallData> = grouped
        .into_values()
        .map(|(item, mut ranges)| {
            ranges
                .sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
            ranges.truncate(CAP);
            CallHierarchyCallData::new(item, ranges)
        })
        .collect();
    out.sort_by(|left, right| {
        left.item
            .path
            .cmp(&right.item.path)
            .then(left.item.span.start.cmp(&right.item.span.start))
    });
    out.truncate(CAP);
    Some(out)
}

/// Callees of the function at `offset`.
///
/// Scans the target body for plain and member calls and resolves each
/// through the same scope and receiver lineage as goto. Builtins and
/// unknown names stay silent. Same file targets only in V1: cross file
/// prepare still answers, but outgoing lists empty. Caps at
/// `MAX_CALL_HIERARCHY`.
pub fn outgoing_in(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Vec<CallHierarchyCallData>> {
    let (target_path, target) = resolve_target_function(file, text, offset, current, deps)?;
    if target_path.as_path() != current {
        return Some(Vec::new());
    }
    let Some(body) = target.body() else {
        return Some(Vec::new());
    };
    let scope = collect_symbols(file);
    let mut grouped: HashMap<(PathBuf, u32, u32), (CallHierarchyItemData, Vec<Span>)> =
        HashMap::new();
    for call in body.all_calls() {
        if grouped.len() >= CAP {
            break;
        }
        let Some(callee) = call.callee() else {
            continue;
        };
        if callee.is_empty() {
            continue;
        }
        let anchor = call.syntax().span.start;
        if is_guarded_builtin(file, text, anchor, &scope) {
            continue;
        }
        let Some((path, found)) =
            resolve_plain_function(file, &scope, deps, anchor, &callee, current)
        else {
            continue;
        };
        let Some(item) = function_item(path.clone(), &found) else {
            continue;
        };
        let key = (item.path.clone(), item.span.start, item.span.end);
        grouped
            .entry(key)
            .or_insert_with(|| (item, Vec::new()))
            .1
            .push(call.syntax().span);
    }
    for member in body.all_members() {
        if grouped.len() >= CAP {
            break;
        }
        if !member.is_call() {
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
        let anchor = member.syntax().span.start;
        if is_guarded_builtin(file, text, anchor, &scope) {
            continue;
        }
        let kind = super::members::resolve_chained_kind(file, &scope, anchor, &chain, deps);
        let super::members::TypeKind::Contract(target_name) = kind else {
            continue;
        };
        if target_name.is_empty() {
            continue;
        }
        let Some((def, dep_path)) =
            super::inheritance::find_contract_across(&target_name, file, deps)
        else {
            continue;
        };
        let mut defs = Vec::new();
        push_plain_functions(&def, &name, &mut defs);
        for base in super::inheritance::linearize(file, &target_name) {
            push_plain_functions(&base, &name, &mut defs);
        }
        for (base, _) in super::inheritance::linearize_across(file, &target_name, deps) {
            push_plain_functions(&base, &name, &mut defs);
        }
        let Some(found) = defs.into_iter().next() else {
            continue;
        };
        let path = dep_path.unwrap_or_else(|| current.to_path_buf());
        let Some(item) = function_item(path.clone(), &found) else {
            continue;
        };
        let key = (item.path.clone(), item.span.start, item.span.end);
        let site = member.member_span().unwrap_or(member.syntax().span);
        grouped
            .entry(key)
            .or_insert_with(|| (item, Vec::new()))
            .1
            .push(site);
    }
    let mut out: Vec<CallHierarchyCallData> = grouped
        .into_values()
        .map(|(item, mut ranges)| {
            ranges
                .sort_by(|left, right| left.start.cmp(&right.start).then(left.end.cmp(&right.end)));
            ranges.truncate(CAP);
            CallHierarchyCallData::new(item, ranges)
        })
        .collect();
    out.sort_by(|left, right| {
        left.item
            .path
            .cmp(&right.item.path)
            .then(left.item.span.start.cmp(&right.item.span.start))
    });
    out.truncate(CAP);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgels_core::routing::Language;
    use forgels_syntax::Lexer;

    fn workspace_with(files: &[(&str, &str)]) -> forgels_core::Workspace {
        let mut workspace = forgels_core::Workspace::new();
        for (name, text) in files {
            workspace
                .open(
                    PathBuf::from(name),
                    Language::Solidity,
                    (*text).to_string(),
                    1,
                )
                .expect("open works");
        }
        workspace
    }

    fn parse(text: &str) -> SolidityFile {
        let tokens = crate::SolidityLexer.tokenize(text);
        let (file, _) = crate::SolidityParser.parse_with_errors(&tokens);
        file
    }

    fn prepare_at(
        _workspace: &forgels_core::Workspace,
        path: &str,
        text: &str,
        needle: &str,
    ) -> Option<CallHierarchyItemData> {
        let buf = PathBuf::from(path);
        let at = text.rfind(needle).expect("needle") as u32;
        let file = parse(text);
        prepare_call_in(&file, text, at, buf.as_path(), &[])
    }

    #[test]
    fn prepare_resolves_function() {
        let text = "contract C {\nfunction greet(uint a) public {}\nfunction f() public {\ngreet(1);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let found = prepare_at(&workspace, "/A.sol", text, "greet(1)").expect("item");
        assert_eq!(found.name, "greet");
    }

    #[test]
    fn prepare_skips_unknown_and_builtin() {
        let text = "contract C {\nfunction f() public {\nmissing(1);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        assert!(prepare_at(&workspace, "/A.sol", text, "missing").is_none());
        let builtin = "contract T is Test {\nfunction f() public {\nassertEq(1, 2);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", builtin)]);
        assert!(prepare_at(&workspace, "/A.sol", builtin, "assertEq").is_none());
    }

    #[test]
    fn incoming_groups_callers() {
        let text = "contract C {\nfunction greet(uint a) public {}\nfunction a() public {\ngreet(1);\n}\nfunction b() public {\ngreet(2);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let buf = PathBuf::from("/A.sol");
        let at = text.find("function greet").expect("decl") as u32 + 9;
        let ctx = ProjectContext::new(buf.as_path(), text, at, &workspace);
        let file = parse(text);
        let found = incoming_in(&file, text, at, buf.as_path(), &[], &ctx).expect("incoming");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn outgoing_lists_callees() {
        let text = "contract C {\nfunction greet(uint a) public {}\nfunction settle(uint a) public {}\nfunction f() public {\ngreet(1);\nsettle(2);\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let _ = workspace;
        let buf = PathBuf::from("/A.sol");
        let at = text.find("function f").expect("decl") as u32 + 9;
        let file = parse(text);
        let found = outgoing_in(&file, text, at, buf.as_path(), &[]).expect("outgoing");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn outgoing_lists_nested_callees_once() {
        let text = "contract C {\nfunction greet(uint a) public {}\nfunction settle(uint a) public {}\nfunction f() public {\nif (true) {\ngreet(1);\n}\nwhile (true) {\nsettle(2);\n}\n}\n}";
        let workspace = workspace_with(&[("/A.sol", text)]);
        let _ = workspace;
        let buf = PathBuf::from("/A.sol");
        let at = text.find("function f").expect("decl") as u32 + 9;
        let file = parse(text);
        let found = outgoing_in(&file, text, at, buf.as_path(), &[]).expect("outgoing");
        assert_eq!(found.len(), 2);
    }
}
