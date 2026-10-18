use std::collections::HashSet;
use std::path::{Path, PathBuf};

use forgels_syntax::{AstNode, Span};

use super::{Scope, SymbolKind, address, array, bytes, function, globals, inheritance};
use crate::{SolidityFile, TypeNode};

/// Receiver type for member access V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Contract(String),
    Struct(String),
    Enum(String),
    UserType(String),
    Super { enclosing: String },
    Array { dynamic: bool },
    Address { payable: bool },
    Bytes,
    String,
    Function,
    Meta(String),
    Global(String),
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
    // Skip an intermediate call `foo()` before the dot: `a.foo().bar`
    // treats `foo` as the receiver. Balanced parens only, comments
    // ignored by the caller scan.
    let mut cursor = dot;
    while cursor > 0 && bytes[cursor - 1] == b')' {
        let mut depth = 0usize;
        let mut index = cursor - 1;
        loop {
            if bytes[index] == b')' {
                depth += 1;
            } else if bytes[index] == b'(' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            if index == 0 {
                return None;
            }
            index -= 1;
        }
        cursor = index;
        while cursor > 0 && bytes[cursor - 1] == b' ' {
            cursor -= 1;
        }
    }
    let mut start = cursor;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    if start == cursor {
        return None;
    }
    let name = text.get(start..cursor)?.to_string();
    if name.is_empty() {
        return None;
    }
    Some((name, Span::new(start as u32, cursor as u32)))
}

/// Member context at `offset` via text scan.
///
/// Covers `p.age` on the member word and `p.` plus `p.ag` after the dot.
/// Dots with spaces around them return `None`. Chained `a.b.c` returns
/// the final segment only (`b` as receiver, `c` as member); callers
/// resolve the full chain with `chain_before` plus `resolve_chained_kind`.
/// Intermediate calls (`a.foo().bar`) skip the parens and treat `foo`
/// as the receiver.
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

fn var_type_of_detail(detail: &str) -> Option<String> {
    let without_value = detail
        .split_once(" = ")
        .map(|pair| pair.0)
        .unwrap_or(detail);
    let index = without_value.rfind(' ')?;
    let left = without_value.get(..index)?.trim();
    if left.is_empty() {
        return None;
    }
    Some(left.to_string())
}

/// Classify the receiver: `this` is the enclosing contract, `super`
/// is the enclosing contract bases, variables use their declared type,
/// and anything else stays unknown.
///
/// `super` bypasses the derived contract: only base members resolve.
/// Array types use the outer suffix only: `uint[2][]` is dynamic while
/// `uint[][2]` is fixed. Data location (`memory`, `storage`, `calldata`)
/// is already stripped from the stored type, so V1 ignores it. Address
/// check runs after the array check so `address[]` stays an array.
/// `address payable` sets `payable` true, plain `address` sets false.
/// `bytes` and `bytes32` share one set with `length` only, `string` has
/// no members by design. External `function` types yield `Function`,
/// internal ones stay unknown. Named functions resolve as `Function` so
/// `f.selector` works. Enum and user type names resolve as `Enum` and
/// `UserType` so `Status.Active` and `MyNumber.wrap` work. Undeclared
/// `msg`, `block`, `tx`, and `abi` yield `Global`; user declarations
/// shadow them.
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
        let Some(contract) = contract_at(file, offset) else {
            return TypeKind::Unknown;
        };
        let Some(name) = contract.name() else {
            return TypeKind::Unknown;
        };
        if name.is_empty() {
            return TypeKind::Unknown;
        }
        return TypeKind::Super { enclosing: name };
    }
    let Some(symbol) = scope.resolve(receiver, offset) else {
        if globals::is_global_receiver(receiver) {
            return TypeKind::Global(receiver.to_string());
        }
        return TypeKind::Unknown;
    };
    if symbol.kind == SymbolKind::Function {
        return TypeKind::Function;
    }
    if symbol.kind == SymbolKind::Enum {
        return TypeKind::Enum(symbol.name.clone());
    }
    if symbol.kind == SymbolKind::UserType {
        return TypeKind::UserType(symbol.name.clone());
    }
    if !matches!(
        symbol.kind,
        SymbolKind::StateVar | SymbolKind::LocalVar | SymbolKind::Param
    ) {
        return TypeKind::Unknown;
    }
    let Some(type_text) = var_type_of_detail(&symbol.detail) else {
        return TypeKind::Unknown;
    };
    if let Some(dynamic) = array::array_shape(&type_text) {
        return TypeKind::Array { dynamic };
    }
    if address::is_address_type(&type_text) {
        return TypeKind::Address {
            payable: address::is_payable_address(&type_text),
        };
    }
    if bytes::is_bytes_type(&type_text) {
        return TypeKind::Bytes;
    }
    if bytes::is_string_type(&type_text) {
        return TypeKind::String;
    }
    if function::is_external_function_type(&type_text) {
        return TypeKind::Function;
    }
    if !is_single_word(&type_text) {
        return TypeKind::Unknown;
    }
    if inheritance::find_contract(file, &type_text).is_some() {
        return TypeKind::Contract(type_text);
    }
    if inheritance::find_struct(file, &type_text).is_some() {
        return TypeKind::Struct(type_text);
    }
    TypeKind::Unknown
}

/// Classify the receiver with deps: same file first, then dep files.
///
/// `super` and `this` resolve locally only. Dep lookup lets `Hello h;
/// h.greet` resolve when `Hello` lives in an import, and lets
/// `Status.Active` resolve when the enum lives in an import.
/// Arrays, addresses, bytes, strings, functions, and globals resolve
/// locally via `receiver_type`, so no dep lookup applies.
pub(crate) fn receiver_type_with_deps(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    receiver: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> TypeKind {
    let local = receiver_type(file, scope, offset, receiver);
    if local != TypeKind::Unknown {
        return local;
    }
    if receiver == "this" || receiver == "super" {
        return TypeKind::Unknown;
    }
    let Some(symbol) = scope.resolve(receiver, offset) else {
        if inheritance::find_enum_across(receiver, file, deps).is_some() {
            return TypeKind::Enum(receiver.to_string());
        }
        if inheritance::find_user_type_across(receiver, file, deps).is_some() {
            return TypeKind::UserType(receiver.to_string());
        }
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
    if inheritance::find_contract_across(&type_text, file, deps).is_some() {
        return TypeKind::Contract(type_text);
    }
    if inheritance::find_struct_across(&type_text, file, deps).is_some() {
        return TypeKind::Struct(type_text);
    }
    TypeKind::Unknown
}

/// Full ident chain before the dot at `dot`.
///
/// Walks backwards through `.` separated words, skipping intermediate
/// `(...)` calls. Capped at receiver plus first member plus
/// `MAX_CHAIN_TAILS` tails (six idents max). Returns root first, so
/// `a.b.c` gives `["a", "b"]` for the dot before `c`.
pub fn chain_before(text: &str, dot: u32) -> Vec<String> {
    use crate::MAX_CHAIN_TAILS;
    let bytes = text.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut cursor = dot as usize;
    let cap = MAX_CHAIN_TAILS + 2;
    loop {
        if out.len() >= cap {
            break;
        }
        if cursor == 0 || cursor > bytes.len() {
            break;
        }
        let mut probe = cursor;
        while probe > 0 && bytes[probe - 1] == b')' {
            let mut depth = 0usize;
            let mut index = probe - 1;
            let mut found = None;
            loop {
                if bytes[index] == b')' {
                    depth += 1;
                } else if bytes[index] == b'(' {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        found = Some(index);
                        break;
                    }
                }
                if index == 0 {
                    break;
                }
                index -= 1;
            }
            let Some(open) = found else {
                return out.into_iter().rev().collect();
            };
            probe = open;
            while probe > 0 && bytes[probe - 1] == b' ' {
                probe -= 1;
            }
        }
        let mut start = probe;
        while start > 0 && is_word_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start == probe {
            break;
        }
        let Some(word) = text.get(start..probe).map(|part| part.to_string()) else {
            break;
        };
        if word.is_empty() {
            break;
        }
        out.push(word);
        if start == 0 || bytes[start - 1] != b'.' {
            break;
        }
        cursor = start - 1;
    }
    out.into_iter().rev().collect()
}

/// Map a declared type text to a `TypeKind` with dep fallback.
///
/// Covers arrays, addresses, bytes, strings, external functions, then
/// single-word contracts, structs, enums, and user types across files.
/// Anything else stays unknown.
fn kind_of_type_text(
    file: &SolidityFile,
    type_text: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> TypeKind {
    if let Some(dynamic) = array::array_shape(type_text) {
        return TypeKind::Array { dynamic };
    }
    if address::is_address_type(type_text) {
        return TypeKind::Address {
            payable: address::is_payable_address(type_text),
        };
    }
    if bytes::is_bytes_type(type_text) {
        return TypeKind::Bytes;
    }
    if bytes::is_string_type(type_text) {
        return TypeKind::String;
    }
    if function::is_external_function_type(type_text) {
        return TypeKind::Function;
    }
    if !is_single_word(type_text) {
        return TypeKind::Unknown;
    }
    if inheritance::find_contract_across(type_text, file, deps).is_some() {
        return TypeKind::Contract(type_text.to_string());
    }
    if inheritance::find_struct_across(type_text, file, deps).is_some() {
        return TypeKind::Struct(type_text.to_string());
    }
    if inheritance::find_enum_across(type_text, file, deps).is_some() {
        return TypeKind::Enum(type_text.to_string());
    }
    if inheritance::find_user_type_across(type_text, file, deps).is_some() {
        return TypeKind::UserType(type_text.to_string());
    }
    TypeKind::Unknown
}

/// Declared type of `member` inside a contract parent.
///
/// Searches own state vars and functions, then linearized bases same
/// file plus across files. State vars use their type text; functions
/// use their first return type (intermediate calls are skipped, so the
/// return counts). Getters and using members stay out of scope for
/// intermediates and yield unknown.
fn contract_member_kind(
    file: &SolidityFile,
    target: &str,
    member: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<TypeKind> {
    if target.is_empty() || member.is_empty() {
        return None;
    }
    let (def, _) = inheritance::find_contract_across(target, file, deps)?;
    for var in def.state_vars() {
        if var.name().as_deref() == Some(member) {
            let type_text = var.type_node().map(|node| node.text()).unwrap_or_default();
            if type_text.is_empty() {
                return Some(TypeKind::Unknown);
            }
            return Some(kind_of_type_text(file, &type_text, deps));
        }
    }
    for function in def.functions() {
        if function.name().as_deref() == Some(member) {
            let returns = function.return_types();
            let Some(first) = returns.first() else {
                return Some(TypeKind::Unknown);
            };
            let type_text = first.text();
            if type_text.is_empty() {
                return Some(TypeKind::Unknown);
            }
            return Some(kind_of_type_text(file, &type_text, deps));
        }
    }
    for base in inheritance::linearize(file, target) {
        for var in base.state_vars() {
            if var.name().as_deref() == Some(member) {
                let type_text = var.type_node().map(|node| node.text()).unwrap_or_default();
                if type_text.is_empty() {
                    return Some(TypeKind::Unknown);
                }
                return Some(kind_of_type_text(file, &type_text, deps));
            }
        }
        for function in base.functions() {
            if function.name().as_deref() == Some(member) {
                let returns = function.return_types();
                let Some(first) = returns.first() else {
                    return Some(TypeKind::Unknown);
                };
                let type_text = first.text();
                if type_text.is_empty() {
                    return Some(TypeKind::Unknown);
                }
                return Some(kind_of_type_text(file, &type_text, deps));
            }
        }
    }
    for (base, _) in inheritance::linearize_across(file, target, deps) {
        for var in base.state_vars() {
            if var.name().as_deref() == Some(member) {
                let type_text = var.type_node().map(|node| node.text()).unwrap_or_default();
                if type_text.is_empty() {
                    return Some(TypeKind::Unknown);
                }
                return Some(kind_of_type_text(file, &type_text, deps));
            }
        }
        for function in base.functions() {
            if function.name().as_deref() == Some(member) {
                let returns = function.return_types();
                let Some(first) = returns.first() else {
                    return Some(TypeKind::Unknown);
                };
                let type_text = first.text();
                if type_text.is_empty() {
                    return Some(TypeKind::Unknown);
                }
                return Some(kind_of_type_text(file, &type_text, deps));
            }
        }
    }
    None
}

/// Declared type of `member` inside a struct parent across files.
fn struct_member_kind(
    file: &SolidityFile,
    target: &str,
    member: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<TypeKind> {
    if target.is_empty() || member.is_empty() {
        return None;
    }
    let (def, _) = inheritance::find_struct_across(target, file, deps)?;
    for field in def.fields() {
        if field.name().as_deref() == Some(member) {
            let type_text = field
                .type_node()
                .map(|node| node.text())
                .unwrap_or_default();
            if type_text.is_empty() {
                return Some(TypeKind::Unknown);
            }
            return Some(kind_of_type_text(file, &type_text, deps));
        }
    }
    None
}

/// True for builtin value kinds that terminate chain walking.
///
/// Arrays, addresses, bytes, strings, functions, metadata, and globals
/// have no contract lineage: further intermediates stay silent and the
/// final segment resolves in the builtin table only.
fn is_builtin_kind(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Array { .. }
            | TypeKind::Address { .. }
            | TypeKind::Bytes
            | TypeKind::String
            | TypeKind::Function
            | TypeKind::Meta(_)
            | TypeKind::Global(_)
    )
}

/// Kind of the last element in a root-first chain.
///
/// `["o"]` gives the receiver type; `["o", "inner"]` gives the declared
/// type of `inner`; longer chains walk per level with cross-file lookup.
/// Unknown members and builtin intermediates yield unknown.
pub fn kind_of_chain(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    chain: &[String],
    deps: &[(PathBuf, SolidityFile)],
) -> TypeKind {
    let Some(root) = chain.first() else {
        return TypeKind::Unknown;
    };
    let mut kind = receiver_type_with_deps(file, scope, offset, root, deps);
    if chain.len() == 1 {
        return kind;
    }
    for member in &chain[1..] {
        if matches!(kind, TypeKind::Unknown) || is_builtin_kind(&kind) {
            return TypeKind::Unknown;
        }
        let next = match &kind {
            TypeKind::Contract(target) => contract_member_kind(file, target, member, deps),
            TypeKind::Struct(target) => struct_member_kind(file, target, member, deps),
            TypeKind::Super { enclosing } => {
                let mut found = None;
                for base in inheritance::linearize(file, enclosing) {
                    let Some(base_name) = base.name() else {
                        continue;
                    };
                    if base_name.is_empty() {
                        continue;
                    }
                    if let Some(next) = contract_member_kind(file, &base_name, member, deps) {
                        found = Some(next);
                        break;
                    }
                }
                found
            }
            _ => None,
        };
        let Some(next) = next else {
            return TypeKind::Unknown;
        };
        kind = next;
    }
    kind
}

/// Parent kind for the final segment of a chain.
///
/// Walks `chain` root first: root via `receiver_type_with_deps`, each
/// intermediate via its declared type with cross-file lookup per level.
/// Unknown members and builtin intermediates yield unknown (intermediate
/// silent). Builtins terminate: no further lineage is followed.
pub fn resolve_chained_kind(
    file: &SolidityFile,
    scope: &Scope,
    offset: u32,
    chain: &[String],
    deps: &[(PathBuf, SolidityFile)],
) -> TypeKind {
    if chain.len() <= 2 {
        let Some(root) = chain.first() else {
            return TypeKind::Unknown;
        };
        if chain.len() < 2 {
            return TypeKind::Unknown;
        }
        return receiver_type_with_deps(file, scope, offset, root, deps);
    }
    let prefix = &chain[..chain.len() - 1];
    kind_of_chain(file, scope, offset, prefix, deps)
}

/// Parent kind for a member context with chained fallback.
///
/// Uses `chain_before` prefix kind when it holds more than one ident;
/// otherwise falls back to single-level `receiver_type_with_deps`.
/// Final-segment-only: only the last member resolves, intermediates
/// stay silent.
pub fn parent_kind_for_member(
    file: &SolidityFile,
    scope: &Scope,
    text: &str,
    offset: u32,
    ctx: &MemberContext,
    deps: &[(PathBuf, SolidityFile)],
) -> TypeKind {
    let prefix = chain_before(text, ctx.dot_pos);
    if prefix.len() >= 2 {
        let resolved = kind_of_chain(file, scope, offset, &prefix, deps);
        if resolved != TypeKind::Unknown {
            return resolved;
        }
        return TypeKind::Unknown;
    }
    receiver_type_with_deps(file, scope, offset, &ctx.receiver, deps)
}

/// Wrap and unwrap entries for a user defined value type.
///
/// Both are synthetic with no declaration span: callers show the cursor
/// member span for hover and return `None` for goto. Empty when the
/// definition lacks a name or an underlying type.
fn user_type_members(item: &crate::UserTypeDef) -> Vec<MemberInfo> {
    let Some(name) = item.name() else {
        return Vec::new();
    };
    if name.is_empty() {
        return Vec::new();
    }
    let Some(underlying) = item.underlying().map(|node| node.text()) else {
        return Vec::new();
    };
    if underlying.is_empty() {
        return Vec::new();
    }
    let span = Span::new(0, 0);
    vec![
        MemberInfo {
            name: String::from("wrap"),
            detail: format!("function wrap({underlying} value) returns ({name})"),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("unwrap"),
            detail: format!("function unwrap({name} value) returns ({underlying})"),
            kind: SymbolKind::Function,
            span,
        },
    ]
}

/// Members of a contract (state vars plus functions), struct (fields),
/// enum (variants), user type (`wrap` and `unwrap`), super (base vars
/// plus base functions, derived bypassed), array (`length` always,
/// `push` and `pop` for dynamic only), address (payable eight, plain
/// six), bytes (`length` only, string has none), function
/// (`selector`, `gas`, `value`), globals (`msg`, `block`, `tx`, `abi`
/// tables), or `type()` metadata by inner. Unknown types yield no
/// members. Contract members include same file inheritance with
/// ` - from Base` origin, derived wins.
pub fn members_of(file: &SolidityFile, kind: &TypeKind) -> Vec<MemberInfo> {
    match kind {
        TypeKind::Unknown => Vec::new(),
        TypeKind::Array { dynamic } => array::array_members(*dynamic),
        TypeKind::Address { payable } => address::address_members(*payable),
        TypeKind::Bytes => bytes::bytes_members(),
        TypeKind::String => bytes::string_members(),
        TypeKind::Function => function::function_members(),
        TypeKind::Meta(inner) => super::meta::meta_members_for(inner),
        TypeKind::Global(name) => globals::global_members(name),
        TypeKind::Super { enclosing } => inheritance::inherited_members(file, enclosing),
        TypeKind::Enum(name) => {
            let Some(item) = inheritance::find_enum(file, name) else {
                return Vec::new();
            };
            item.variant_entries()
                .into_iter()
                .map(|(variant, span)| MemberInfo {
                    name: variant.clone(),
                    detail: format!("{name}.{variant}"),
                    kind: SymbolKind::EnumMember,
                    span,
                })
                .collect()
        }
        TypeKind::UserType(name) => {
            let Some(item) = inheritance::find_user_type(file, name) else {
                return Vec::new();
            };
            user_type_members(&item)
        }
        TypeKind::Contract(name) => {
            let Some(contract) = inheritance::find_contract(file, name) else {
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
                if let Some(init) = var.initializer() {
                    detail.push_str(" = ");
                    detail.push_str(&init.text);
                }
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
            for inherited in inheritance::inherited_members(file, name) {
                if out.iter().any(|own| own.name == inherited.name) {
                    continue;
                }
                out.push(inherited);
            }
            for getter in super::getter::getter_members(&contract) {
                if out
                    .iter()
                    .any(|own| own.name == getter.name && own.kind == SymbolKind::Function)
                {
                    continue;
                }
                out.push(getter);
            }
            out
        }
        TypeKind::Struct(name) => {
            let Some(item) = inheritance::find_struct(file, name) else {
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

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown.sol")
        .to_string()
}

fn own_member_info(
    base_name: &str,
    file_name: &str,
    var_name: String,
    var_detail: String,
    kind: SymbolKind,
    span: forgels_syntax::Span,
) -> MemberInfo {
    let mut detail = var_detail;
    detail.push_str(" - from ");
    detail.push_str(base_name);
    detail.push_str(" in ");
    detail.push_str(file_name);
    MemberInfo {
        name: var_name,
        detail,
        kind,
        span,
    }
}

/// All dep members for `kind` with ` - from Base in file` origin.
///
/// Skips names already present locally so derived wins. Used for member
/// completion across imports. Deps come from `dep_texts` in index order.
/// Super skips same file bases (`None` paths): they are already local.
/// User type members are synthetic, so dep entries only fill in when the
/// primary file lacks the definition.
pub(crate) fn collect_cross_file_members(
    primary: &SolidityFile,
    kind: &TypeKind,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<(MemberInfo, PathBuf)> {
    let local = members_of(primary, kind);
    let mut seen: HashSet<String> = local.iter().map(|item| item.name.clone()).collect();
    let mut out: Vec<(MemberInfo, PathBuf)> = Vec::new();
    match kind {
        TypeKind::Unknown => Vec::new(),
        // Builtin value types have no dep members: arrays, addresses,
        // bytes, strings, functions, globals, and type() metadata.
        TypeKind::Array { .. } => Vec::new(),
        TypeKind::Address { .. } => Vec::new(),
        TypeKind::Bytes => Vec::new(),
        TypeKind::String => Vec::new(),
        TypeKind::Function => Vec::new(),
        TypeKind::Meta(_) => Vec::new(),
        TypeKind::Global(_) => Vec::new(),
        TypeKind::Contract(name) => {
            if let Some((direct, Some(path))) =
                inheritance::find_contract_across(name, primary, deps)
            {
                let file_name = file_label(&path);
                let Some(base_name) = direct.name() else {
                    return out;
                };
                for var in direct.state_vars() {
                    let Some(var_name) = var.name() else {
                        continue;
                    };
                    if var_name.is_empty() || !seen.insert(var_name.clone()) {
                        continue;
                    }
                    let mut base_detail =
                        var.type_node().map(|node| node.text()).unwrap_or_default();
                    base_detail.push(' ');
                    base_detail.push_str(&var_name);
                    if let Some(init) = var.initializer() {
                        base_detail.push_str(" = ");
                        base_detail.push_str(&init.text);
                    }
                    out.push((
                        own_member_info(
                            &base_name,
                            &file_name,
                            var_name,
                            base_detail,
                            SymbolKind::StateVar,
                            var.syntax().span,
                        ),
                        path.clone(),
                    ));
                }
                for function in direct.functions() {
                    let Some(func_name) = function.name() else {
                        continue;
                    };
                    if func_name.is_empty() || !seen.insert(func_name.clone()) {
                        continue;
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
                    let mut base_detail = format!("function {func_name}({})", params.join(", "));
                    let returns: Vec<String> = function
                        .return_types()
                        .iter()
                        .map(|node| node.text())
                        .collect();
                    if !returns.is_empty() {
                        base_detail.push_str(" returns (");
                        base_detail.push_str(&returns.join(", "));
                        base_detail.push(')');
                    }
                    out.push((
                        own_member_info(
                            &base_name,
                            &file_name,
                            func_name,
                            base_detail,
                            SymbolKind::Function,
                            function.syntax().span,
                        ),
                        path.clone(),
                    ));
                }
            }
            for (base, path) in inheritance::linearize_across(primary, name, deps) {
                let Some(path) = path else {
                    continue;
                };
                let file_name = file_label(&path);
                let Some(base_name) = base.name() else {
                    continue;
                };
                if base_name.is_empty() {
                    continue;
                }
                for var in base.state_vars() {
                    let Some(var_name) = var.name() else {
                        continue;
                    };
                    if var_name.is_empty() || !seen.insert(var_name.clone()) {
                        continue;
                    }
                    let mut base_detail =
                        var.type_node().map(|node| node.text()).unwrap_or_default();
                    base_detail.push(' ');
                    base_detail.push_str(&var_name);
                    if let Some(init) = var.initializer() {
                        base_detail.push_str(" = ");
                        base_detail.push_str(&init.text);
                    }
                    out.push((
                        own_member_info(
                            &base_name,
                            &file_name,
                            var_name,
                            base_detail,
                            SymbolKind::StateVar,
                            var.syntax().span,
                        ),
                        path.clone(),
                    ));
                }
                for function in base.functions() {
                    let Some(func_name) = function.name() else {
                        continue;
                    };
                    if func_name.is_empty() || !seen.insert(func_name.clone()) {
                        continue;
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
                    let mut base_detail = format!("function {func_name}({})", params.join(", "));
                    let returns: Vec<String> = function
                        .return_types()
                        .iter()
                        .map(|node| node.text())
                        .collect();
                    if !returns.is_empty() {
                        base_detail.push_str(" returns (");
                        base_detail.push_str(&returns.join(", "));
                        base_detail.push(')');
                    }
                    out.push((
                        own_member_info(
                            &base_name,
                            &file_name,
                            func_name,
                            base_detail,
                            SymbolKind::Function,
                            function.syntax().span,
                        ),
                        path.clone(),
                    ));
                }
            }
            out
        }
        TypeKind::Struct(name) => {
            if local.iter().any(|item| item.name == *name) {
                return out;
            }
            let Some((item, Some(path))) = inheritance::find_struct_across(name, primary, deps)
            else {
                return out;
            };
            let file_name = file_label(&path);
            let Some(struct_name) = item.name() else {
                return out;
            };
            for field in item.fields() {
                let Some(field_name) = field.name() else {
                    continue;
                };
                if field_name.is_empty() || !seen.insert(field_name.clone()) {
                    continue;
                }
                let mut base_detail = field
                    .type_node()
                    .map(|node| node.text())
                    .unwrap_or_default();
                base_detail.push(' ');
                base_detail.push_str(&field_name);
                out.push((
                    own_member_info(
                        &struct_name,
                        &file_name,
                        field_name,
                        base_detail,
                        SymbolKind::Field,
                        field.syntax().span,
                    ),
                    path.clone(),
                ));
            }
            out
        }
        TypeKind::Super { enclosing } => {
            for (base, path) in inheritance::linearize_across(primary, enclosing, deps) {
                let Some(path) = path else {
                    continue;
                };
                let file_name = file_label(&path);
                let Some(base_name) = base.name() else {
                    continue;
                };
                if base_name.is_empty() {
                    continue;
                }
                for var in base.state_vars() {
                    let Some(var_name) = var.name() else {
                        continue;
                    };
                    if var_name.is_empty() || !seen.insert(var_name.clone()) {
                        continue;
                    }
                    let mut base_detail =
                        var.type_node().map(|node| node.text()).unwrap_or_default();
                    base_detail.push(' ');
                    base_detail.push_str(&var_name);
                    if let Some(init) = var.initializer() {
                        base_detail.push_str(" = ");
                        base_detail.push_str(&init.text);
                    }
                    out.push((
                        own_member_info(
                            &base_name,
                            &file_name,
                            var_name,
                            base_detail,
                            SymbolKind::StateVar,
                            var.syntax().span,
                        ),
                        path.clone(),
                    ));
                }
                for function in base.functions() {
                    let Some(func_name) = function.name() else {
                        continue;
                    };
                    if func_name.is_empty() || !seen.insert(func_name.clone()) {
                        continue;
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
                    let mut base_detail = format!("function {func_name}({})", params.join(", "));
                    let returns: Vec<String> = function
                        .return_types()
                        .iter()
                        .map(|node| node.text())
                        .collect();
                    if !returns.is_empty() {
                        base_detail.push_str(" returns (");
                        base_detail.push_str(&returns.join(", "));
                        base_detail.push(')');
                    }
                    out.push((
                        own_member_info(
                            &base_name,
                            &file_name,
                            func_name,
                            base_detail,
                            SymbolKind::Function,
                            function.syntax().span,
                        ),
                        path.clone(),
                    ));
                }
            }
            out
        }
        TypeKind::Enum(name) => {
            let Some((item, Some(path))) = inheritance::find_enum_across(name, primary, deps)
            else {
                return out;
            };
            let file_name = file_label(&path);
            for (variant, span) in item.variant_entries() {
                if variant.is_empty() || !seen.insert(variant.clone()) {
                    continue;
                }
                let mut detail = format!("{name}.{variant}");
                detail.push_str(" - from ");
                detail.push_str(&file_name);
                out.push((
                    MemberInfo {
                        name: variant,
                        detail,
                        kind: SymbolKind::EnumMember,
                        span,
                    },
                    path.clone(),
                ));
            }
            out
        }
        TypeKind::UserType(name) => {
            if !local.is_empty() {
                return out;
            }
            let Some((item, Some(path))) = inheritance::find_user_type_across(name, primary, deps)
            else {
                return out;
            };
            let file_name = file_label(&path);
            for info in user_type_members(&item) {
                if !seen.insert(info.name.clone()) {
                    continue;
                }
                let mut detail = info.detail.clone();
                detail.push_str(" - from ");
                detail.push_str(&file_name);
                out.push((
                    MemberInfo {
                        name: info.name.clone(),
                        detail,
                        kind: info.kind,
                        span: info.span,
                    },
                    path.clone(),
                ));
            }
            out
        }
    }
}

/// One dep member by exact name with its owning path.
///
/// Returns `None` when the member lives locally so the caller keeps the
/// same file target. Powers cross file hover and goto.
pub(crate) fn resolve_member_cross_file(
    primary: &SolidityFile,
    kind: &TypeKind,
    member: &str,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<(MemberInfo, PathBuf)> {
    if member.is_empty() {
        return None;
    }
    if members_of(primary, kind)
        .iter()
        .any(|own| own.name == member)
    {
        return None;
    }
    collect_cross_file_members(primary, kind, deps)
        .into_iter()
        .find(|(info, _)| info.name == member)
}
