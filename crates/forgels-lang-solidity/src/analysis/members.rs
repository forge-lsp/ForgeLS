use forgels_syntax::{AstNode, Span};

use super::{Scope, SymbolKind};
use crate::{SolidityFile, TypeNode};

/// Receiver type for member access V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Contract(String),
    Struct(String),
    Unknown,
}

/// Member position after a dot: receiver plus member word or prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberContext {
    pub receiver: String,
    pub receiver_span: Span,
    pub member: String,
    pub member_span: Span,
    pub dot_pos: u32,
}

/// One member for completion, hover, and goto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberInfo {
    pub name: String,
    pub detail: String,
    pub kind: SymbolKind,
    pub span: Span,
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn receiver_before(text: &str, dot: usize) -> Option<(String, Span)> {
    let bytes = text.as_bytes();
    if dot == 0 || dot > bytes.len() {
        return None;
    }
    let mut start = dot;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    if start == dot {
        return None;
    }
    // Chained `a.b.c` is follow-up work: only single dot resolves.
    if start > 0 && bytes[start - 1] == b'.' {
        return None;
    }
    let name = text.get(start..dot)?.to_string();
    if name.is_empty() {
        return None;
    }
    Some((name, Span::new(start as u32, dot as u32)))
}

/// Member context at `offset` via text scan.
///
/// Covers `p.age` on the member word and `p.` plus `p.ag` after the dot.
/// Dots with spaces around them return `None`. Chained `a.b.c` returns
/// `None` so V1 stays single level.
pub fn member_context_at(text: &str, offset: u32) -> Option<MemberContext> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let at = (offset as usize).min(len);
    if at < len && is_word_byte(bytes[at]) {
        let mut start = at;
        while start > 0 && is_word_byte(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = at;
        while end < len && is_word_byte(bytes[end]) {
            end += 1;
        }
        if start == 0 || bytes[start - 1] != b'.' {
            return None;
        }
        let dot = start - 1;
        let (receiver, receiver_span) = receiver_before(text, dot)?;
        let member = text.get(start..end)?.to_string();
        if member.is_empty() {
            return None;
        }
        return Some(MemberContext {
            receiver,
            receiver_span,
            member_span: Span::new(start as u32, end as u32),
            member,
            dot_pos: dot as u32,
        });
    }
    let mut prefix_start = at;
    while prefix_start > 0 && prefix_start <= len && is_word_byte(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    if prefix_start == 0 || bytes[prefix_start - 1] != b'.' {
        return None;
    }
    let dot = prefix_start - 1;
    let (receiver, receiver_span) = receiver_before(text, dot)?;
    let member = text.get(prefix_start..at)?.to_string();
    Some(MemberContext {
        receiver,
        receiver_span,
        member_span: Span::new(prefix_start as u32, at as u32),
        member,
        dot_pos: dot as u32,
    })
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

fn is_single_word(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    text.as_bytes().iter().all(|byte| is_word_byte(*byte))
}

fn find_contract(file: &SolidityFile, name: &str) -> Option<crate::ContractDef> {
    file.contracts()
        .into_iter()
        .find(|contract| contract.name().as_deref() == Some(name))
}

fn find_struct(file: &SolidityFile, name: &str) -> Option<crate::StructDef> {
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

fn var_type_of_detail(detail: &str) -> Option<String> {
    let index = detail.rfind(' ')?;
    let left = detail.get(..index)?.trim();
    if left.is_empty() {
        return None;
    }
    Some(left.to_string())
}

/// Classify the receiver: `this` is the enclosing contract, variables use
/// their declared type, `super` and anything else stay unknown.
pub fn receiver_type(file: &SolidityFile, scope: &Scope, offset: u32, receiver: &str) -> TypeKind {
    if receiver == "this" {
        let Some(contract) = contract_at(file, offset) else {
            return TypeKind::Unknown;
        };
        let Some(name) = contract.name() else {
            return TypeKind::Unknown;
        };
        if name.is_empty() {
            return TypeKind::Unknown;
        }
        return TypeKind::Contract(name);
    }
    if receiver == "super" {
        return TypeKind::Unknown;
    }
    let Some(symbol) = scope.resolve(receiver, offset) else {
        return TypeKind::Unknown;
    };
    if !matches!(
        symbol.kind,
        SymbolKind::StateVar | SymbolKind::LocalVar | SymbolKind::Param
    ) {
        return TypeKind::Unknown;
    }
    let Some(type_text) = var_type_of_detail(&symbol.detail) else {
        return TypeKind::Unknown;
    };
    if !is_single_word(&type_text) {
        return TypeKind::Unknown;
    }
    if find_contract(file, &type_text).is_some() {
        return TypeKind::Contract(type_text);
    }
    if find_struct(file, &type_text).is_some() {
        return TypeKind::Struct(type_text);
    }
    TypeKind::Unknown
}

/// Members of a contract (state vars plus functions) or struct (fields).
/// Unknown types yield no members. Only the current file is searched.
/// Cross-file member lookup is follow-up work.
pub fn members_of(file: &SolidityFile, kind: &TypeKind) -> Vec<MemberInfo> {
    match kind {
        TypeKind::Unknown => Vec::new(),
        TypeKind::Contract(name) => {
            let Some(contract) = find_contract(file, name) else {
                return Vec::new();
            };
            let mut out = Vec::new();
            for var in contract.state_vars() {
                let Some(var_name) = var.name() else {
                    continue;
                };
                if var_name.is_empty() {
                    continue;
                }
                let mut detail = var
                    .type_node()
                    .map(|node| TypeNode::text(&node))
                    .unwrap_or_default();
                detail.push(' ');
                detail.push_str(&var_name);
                out.push(MemberInfo {
                    name: var_name,
                    detail,
                    kind: SymbolKind::StateVar,
                    span: var.syntax().span,
                });
            }
            for function in contract.functions() {
                let Some(func_name) = function.name() else {
                    continue;
                };
                if func_name.is_empty() {
                    continue;
                }
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
                let mut detail = format!("function {func_name}({})", params.join(", "));
                let returns: Vec<String> =
                    function.return_types().iter().map(TypeNode::text).collect();
                if !returns.is_empty() {
                    detail.push_str(" returns (");
                    detail.push_str(&returns.join(", "));
                    detail.push(')');
                }
                out.push(MemberInfo {
                    name: func_name,
                    detail,
                    kind: SymbolKind::Function,
                    span: function.syntax().span,
                });
            }
            out
        }
        TypeKind::Struct(name) => {
            let Some(item) = find_struct(file, name) else {
                return Vec::new();
            };
            let mut out = Vec::new();
            for field in item.fields() {
                let Some(field_name) = field.name() else {
                    continue;
                };
                if field_name.is_empty() {
                    continue;
                }
                let mut detail = field
                    .type_node()
                    .map(|node| TypeNode::text(&node))
                    .unwrap_or_default();
                detail.push(' ');
                detail.push_str(&field_name);
                out.push(MemberInfo {
                    name: field_name,
                    detail,
                    kind: SymbolKind::Field,
                    span: field.syntax().span,
                });
            }
            out
        }
    }
}
