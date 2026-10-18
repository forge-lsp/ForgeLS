use forgels_syntax::AstNode;

use super::members::MemberInfo;
use super::{Symbol, SymbolKind};
use crate::{ContractDef, TypeNode};

/// One synthesized getter: name, params, return type, and decl span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetterInfo {
    pub name: String,
    pub detail: String,
    pub span: forgels_syntax::Span,
    pub params: Vec<String>,
    pub returns: String,
}

fn strip_array_suffixes(type_text: &str) -> String {
    let mut base = type_text.trim().to_string();
    while let Some(open) = base.rfind('[') {
        let Some(close) = base.rfind(']') else {
            break;
        };
        if close < open {
            break;
        }
        base.truncate(open);
        base = base.trim().to_string();
        if base.is_empty() {
            break;
        }
    }
    base
}

fn mapping_parts(type_text: &str) -> Option<(String, String)> {
    let trimmed = type_text.trim();
    let inner = trimmed
        .strip_prefix("mapping(")
        .and_then(|rest| rest.strip_suffix(')'))?;
    let mut depth = 0u32;
    let mut split = None;
    for (index, byte) in inner.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b'=' if depth == 0 && inner.as_bytes().get(index + 1) == Some(&b'>') => {
                split = Some(index);
                break;
            }
            _ => {}
        }
    }
    let at = split?;
    let key = inner.get(..at)?.trim().to_string();
    let value = inner.get(at + 2..)?.trim().to_string();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    if value.contains("mapping(") {
        return None;
    }
    Some((key, value))
}

fn mapping_key_type(key: &str) -> String {
    let mut parts = key.split_whitespace();
    let first = parts.next().unwrap_or("").to_string();
    if first.is_empty() {
        return key.to_string();
    }
    first
}

/// Getter for one public state var, if the type qualifies.
///
/// Scalar yields `name()` with no params. Arrays yield `name(uint256)`
/// with the element as return. Single mappings yield `name(key)` with
/// the value as return. Nested mappings return `None` (deferred).
/// Callers enforce explicit wins: any function with the same name skips
/// synthesis.
pub(crate) fn getter_for(
    var_name: &str,
    type_text: &str,
    span: forgels_syntax::Span,
) -> Option<GetterInfo> {
    if var_name.is_empty() || type_text.is_empty() {
        return None;
    }
    if type_text.trim().starts_with("mapping(") {
        let (key, value) = mapping_parts(type_text)?;
        let key_type = mapping_key_type(&key);
        if key_type.is_empty() {
            return None;
        }
        return Some(GetterInfo {
            name: var_name.to_string(),
            detail: format!("function {var_name}({key_type}) returns ({value})"),
            span,
            params: vec![key_type],
            returns: value,
        });
    }
    if type_text.contains('[') {
        let base = strip_array_suffixes(type_text);
        if base.is_empty() {
            return None;
        }
        return Some(GetterInfo {
            name: var_name.to_string(),
            detail: format!("function {var_name}(uint256) returns ({base})"),
            span,
            params: vec![String::from("uint256")],
            returns: base,
        });
    }
    Some(GetterInfo {
        name: var_name.to_string(),
        detail: format!("function {var_name}() returns ({type_text})"),
        span,
        params: Vec::new(),
        returns: type_text.to_string(),
    })
}

/// All getters in one contract with explicit wins.
///
/// Functions with the same name suppress synthesis, so hand-written
/// getters always win. Private and non-public vars never synthesize.
pub(crate) fn contract_getters(contract: &ContractDef) -> Vec<GetterInfo> {
    let mut explicit: Vec<String> = Vec::new();
    for function in contract.functions() {
        if let Some(name) = function.name()
            && !name.is_empty()
            && !explicit.contains(&name)
        {
            explicit.push(name);
        }
    }
    let mut out = Vec::new();
    for var in contract.state_vars() {
        if !var.is_public() {
            continue;
        }
        let Some(name) = var.name() else {
            continue;
        };
        if name.is_empty() || explicit.contains(&name) {
            continue;
        }
        let type_text = var
            .type_node()
            .map(|node| TypeNode::text(&node))
            .unwrap_or_default();
        if type_text.is_empty() {
            continue;
        }
        let span = var.syntax().span;
        if let Some(getter) = getter_for(&name, &type_text, span) {
            if out
                .iter()
                .any(|known: &GetterInfo| known.name == getter.name)
            {
                continue;
            }
            out.push(getter);
        }
    }
    out
}

/// Getter symbols for scope: function kind with the getter detail.
pub(crate) fn getter_symbols(contract: &ContractDef) -> Vec<Symbol> {
    contract_getters(contract)
        .into_iter()
        .map(|getter| Symbol {
            name: getter.name.clone(),
            kind: SymbolKind::Function,
            span: getter.span,
            detail: getter.detail.clone(),
        })
        .collect()
}

/// Getter members for `this` and contract receivers with explicit wins.
pub(crate) fn getter_members(contract: &ContractDef) -> Vec<MemberInfo> {
    contract_getters(contract)
        .into_iter()
        .map(|getter| MemberInfo {
            name: getter.name.clone(),
            detail: getter.detail.clone(),
            kind: SymbolKind::Function,
            span: getter.span,
        })
        .collect()
}
