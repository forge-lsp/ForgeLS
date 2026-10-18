use std::path::PathBuf;

use forgels_syntax::{AstNode, Span};

use super::inheritance;
use super::{Scope, SymbolKind};
use crate::{SolidityFile, TypeNode};

use super::members::MemberInfo;

fn receiver_type_text(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    receiver: &str,
) -> Option<String> {
    if receiver.is_empty() {
        return None;
    }
    if receiver == "this" {
        for contract in file.contracts() {
            let span = contract.syntax().span;
            if span.contains(offset) || span.end == offset {
                let name = contract.name()?;
                if name.is_empty() {
                    return None;
                }
                return Some(name);
            }
        }
        return None;
    }
    if receiver == "super" {
        return None;
    }
    let symbol = scope.resolve(receiver, offset)?;
    if !matches!(
        symbol.kind,
        SymbolKind::StateVar | SymbolKind::LocalVar | SymbolKind::Param
    ) {
        return None;
    }
    let detail = symbol.detail.as_str();
    let index = detail.rfind(' ')?;
    let left = detail.get(..index)?.trim();
    if left.is_empty() {
        return None;
    }
    Some(left.to_string())
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

fn first_param_type(function: &crate::FunctionDef) -> Option<String> {
    let params = function.params();
    let first = params.first()?;
    first.type_node().map(|node| TypeNode::text(&node))
}

fn function_detail(function: &crate::FunctionDef) -> String {
    let name = function.name().unwrap_or_default();
    let params: Vec<String> = function
        .params()
        .iter()
        .map(|param| {
            let mut text = param
                .type_node()
                .map(|node| TypeNode::text(&node))
                .unwrap_or_default();
            if let Some(param_name) = param.name() {
                text.push(' ');
                text.push_str(&param_name);
            }
            text
        })
        .collect();
    let mut detail = format!("function {name}({})", params.join(", "));
    let returns: Vec<String> = function.return_types().iter().map(TypeNode::text).collect();
    if !returns.is_empty() {
        detail.push_str(" returns (");
        detail.push_str(&returns.join(", "));
        detail.push(')');
    }
    detail
}

/// Every using row span for unknown skip: contract rows plus file rows.
///
/// Operator and multi-library rows keep a span-only node, so their
/// idents never lint even though analysis skips them.
pub fn using_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for using in file.usings() {
        out.push(using.syntax().span);
    }
    for contract in file.contracts() {
        for using in contract.usings() {
            out.push(using.syntax().span);
        }
    }
    out
}

fn usings_for_offset(file: &SolidityFile, offset: u32) -> Vec<crate::UsingDirective> {
    let mut out = Vec::new();
    if let Some(contract) = contract_at(file, offset) {
        out.extend(contract.usings());
    }
    out.extend(file.usings());
    out
}

/// Library functions attached to `receiver` at `offset`.
///
/// Exact match only: the using target must equal the receiver type text,
/// and the library first param must equal the same text. Wildcard `*`
/// matches any receiver whose type equals the library first param.
/// Own members win: callers filter names already present locally.
pub(crate) fn using_members_at(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    receiver: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<(MemberInfo, Option<PathBuf>)> {
    let Some(receiver_type) = receiver_type_text(file, scope, offset, receiver) else {
        return Vec::new();
    };
    if receiver_type.is_empty() {
        return Vec::new();
    }
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<(MemberInfo, Option<PathBuf>)> = Vec::new();
    for using in usings_for_offset(file, offset) {
        let Some(lib) = using.library() else {
            continue;
        };
        if lib.is_empty() {
            continue;
        }
        let wildcard = using.is_wildcard();
        let target = using.target_text().unwrap_or_default();
        if !wildcard && target != receiver_type {
            continue;
        }
        let Some((lib_def, path)) = inheritance::find_contract_across(&lib, file, deps) else {
            continue;
        };
        for function in lib_def.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() || seen.contains(&func_name) {
                continue;
            }
            let Some(first_type) = first_param_type(&function) else {
                continue;
            };
            if first_type != receiver_type {
                continue;
            }
            seen.push(func_name.clone());
            let mut detail = function_detail(&function);
            detail.push_str(" - from ");
            detail.push_str(&lib);
            detail.push_str(" via using");
            out.push((
                MemberInfo {
                    name: func_name,
                    detail,
                    kind: SymbolKind::Function,
                    span: function.syntax().span,
                },
                path.clone(),
            ));
        }
    }
    out
}

/// One attached function by exact member name with its owning path.
///
/// Returns `None` when no using row matches, so callers fall through
/// to own members and builtins. Own precedence is enforced by the
/// caller: check `members_of` first and only then call here.
pub(crate) fn resolve_using_member(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    receiver: &str,
    member: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(MemberInfo, Option<PathBuf>)> {
    if member.is_empty() {
        return None;
    }
    for (info, path) in using_members_at(file, scope, offset, receiver, deps) {
        if info.name == member {
            return Some((info, path));
        }
    }
    None
}

/// Expected call args for an attached member: library params minus one.
///
/// The receiver binds the first library param, so `x.add(1)` checks
/// against `add(self, other)` with expected 1. Returns `None` when no
/// using row provides `member` for `receiver`.
pub(crate) fn using_expected_arity(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    receiver: &str,
    member: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<usize> {
    let receiver_type = receiver_type_text(file, scope, offset, receiver)?;
    for using in usings_for_offset(file, offset) {
        let Some(lib) = using.library() else {
            continue;
        };
        if lib.is_empty() {
            continue;
        }
        let wildcard = using.is_wildcard();
        let target = using.target_text().unwrap_or_default();
        if !wildcard && target != receiver_type {
            continue;
        }
        let (lib_def, _) = inheritance::find_contract_across(&lib, file, deps)?;
        for function in lib_def.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name != member {
                continue;
            }
            let Some(first_type) = first_param_type(&function) else {
                continue;
            };
            if first_type != receiver_type {
                continue;
            }
            let count = function.params().len();
            if count == 0 {
                continue;
            }
            return Some(count.saturating_sub(1));
        }
    }
    None
}
