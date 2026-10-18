use std::path::PathBuf;

use forgels_core::{SignatureData, SignatureParam};
use forgels_syntax::AstNode;

use super::{Scope, SymbolKind, collect_symbols, forge, inheritance, members};
use crate::{ContractDef, EventDef, FunctionDef, Param, SolidityFile};

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Call site under the cursor: callee plus active arg index.
///
/// `anchor` is the open paren byte offset. Scope queries use it
/// instead of the cursor: both sit in one scope, and the anchor stays
/// inside parsed spans while trailing text is still broken.
struct CallSite {
    name: String,
    receiver: Option<String>,
    is_emit: bool,
    active_param: u32,
    anchor: u32,
}

/// True for bytes that belong to code: not strings or comments.
///
/// Forward scan from the file start to `end`. Handles `//` lines,
/// `/* */` blocks, and quoted strings with escapes. Unterminated
/// strings run to `end` so typing never confuses depth counting.
fn code_mask(text: &str, end: usize) -> Vec<bool> {
    let bytes = text.as_bytes();
    let mut mask = vec![true; end];
    let mut index = 0;
    while index < end {
        let byte = bytes[index];
        if byte == b'/' && index + 1 < end && bytes[index + 1] == b'/' {
            mask[index] = false;
            mask[index + 1] = false;
            index += 2;
            while index < end && bytes[index] != b'\n' {
                mask[index] = false;
                index += 1;
            }
            continue;
        }
        if byte == b'/' && index + 1 < end && bytes[index + 1] == b'*' {
            mask[index] = false;
            mask[index + 1] = false;
            index += 2;
            while index < end {
                if bytes[index] == b'*' && index + 1 < end && bytes[index + 1] == b'/' {
                    mask[index] = false;
                    mask[index + 1] = false;
                    index += 2;
                    break;
                }
                mask[index] = false;
                index += 1;
            }
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            mask[index] = false;
            index += 1;
            while index < end {
                if bytes[index] == b'\\' && index + 1 < end {
                    mask[index] = false;
                    mask[index + 1] = false;
                    index += 2;
                    continue;
                }
                if bytes[index] == byte {
                    mask[index] = false;
                    index += 1;
                    break;
                }
                if bytes[index] == b'\n' {
                    break;
                }
                mask[index] = false;
                index += 1;
            }
            continue;
        }
        index += 1;
    }
    mask
}

/// Commas at nesting depth zero between `start` and `cursor`.
fn count_args(mask: &[bool], bytes: &[u8], start: usize, cursor: usize) -> u32 {
    let mut depth: usize = 0;
    let mut count: usize = 0;
    let mut index = start;
    while index < cursor {
        if mask[index] {
            match bytes[index] {
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => count += 1,
                _ => {}
            }
        }
        index += 1;
    }
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Callee before `open`: name, optional receiver, emit flag.
///
/// Accepts `greet(`, `h.greet(`, `this.greet(`, and `emit Transfer(`.
/// Spaces may sit between the name and the paren. Dots must touch
/// both sides, the same stance as member lookup. Returns `None` when
/// no identifier precedes the paren, for example grouping parens.
fn callee_at(text: &str, open: usize) -> Option<(String, Option<String>, bool)> {
    let bytes = text.as_bytes();
    let mut end = open;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    if start == end {
        return None;
    }
    let name = text.get(start..end)?.to_string();
    if start > 0 && bytes[start - 1] == b'.' {
        let dot = start - 1;
        let mut recv_start = dot;
        while recv_start > 0 && is_word_byte(bytes[recv_start - 1]) {
            recv_start -= 1;
        }
        if recv_start == dot {
            return None;
        }
        let receiver = text.get(recv_start..dot)?.to_string();
        return Some((name, Some(receiver), false));
    }
    let mut before = start;
    while before > 0 && bytes[before - 1].is_ascii_whitespace() {
        before -= 1;
    }
    let mut word_start = before;
    while word_start > 0 && is_word_byte(bytes[word_start - 1]) {
        word_start -= 1;
    }
    let is_emit = text.get(word_start..before) == Some("emit");
    Some((name, None, is_emit))
}

/// Innermost call enclosing `offset`, if any.
///
/// Backward scan for the open paren with depth tracking over code
/// bytes only. Stops at `;`, `{`, `}` on depth zero so an earlier
/// closed call never leaks in. Closed calls yield `None`: the first
/// byte behind the cursor raises depth and the paren is consumed.
/// Grouping parens without a callee are skipped outward. Unclosed
/// parens work: nothing ahead of the cursor matters.
fn find_call(text: &str, offset: u32) -> Option<CallSite> {
    let cursor = (offset as usize).min(text.len());
    if cursor == 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mask = code_mask(text, cursor);
    let mut depth: usize = 0;
    let mut index = cursor;
    while index > 0 {
        index -= 1;
        if !mask[index] {
            continue;
        }
        match bytes[index] {
            b')' => depth += 1,
            b'(' => {
                if depth > 0 {
                    depth -= 1;
                    continue;
                }
                let Some((name, receiver, is_emit)) = callee_at(text, index) else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let active = count_args(&mask, bytes, index + 1, cursor);
                return Some(CallSite {
                    name,
                    receiver,
                    is_emit,
                    active_param: active,
                    anchor: index as u32,
                });
            }
            b';' | b'{' | b'}' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

fn contract_at(file: &SolidityFile, offset: u32) -> Option<ContractDef> {
    for contract in file.contracts() {
        let span = contract.syntax().span;
        if span.contains(offset) || span.end == offset {
            return Some(contract);
        }
    }
    None
}

fn param_text(param: &Param) -> String {
    let mut text = param
        .type_node()
        .map(|node| node.text())
        .unwrap_or_default();
    if let Some(name) = param.name() {
        text.push(' ');
        text.push_str(&name);
    }
    text
}

fn function_signature(function: &FunctionDef, active: u32) -> SignatureData {
    let name = function.name().unwrap_or_default();
    let params: Vec<SignatureParam> = function
        .params()
        .iter()
        .map(|param| SignatureParam::new(param_text(param)))
        .collect();
    let labels: Vec<&str> = params.iter().map(|param| param.label.as_str()).collect();
    let mut label = format!("function {name}({})", labels.join(", "));
    let returns: Vec<String> = function
        .return_types()
        .iter()
        .map(|node| node.text())
        .collect();
    if !returns.is_empty() {
        label.push_str(" returns (");
        label.push_str(&returns.join(", "));
        label.push(')');
    }
    SignatureData::new(label, params, active)
}

fn event_signature(event: &EventDef, active: u32) -> SignatureData {
    let name = event.name().unwrap_or_default();
    let params: Vec<SignatureParam> = event
        .params()
        .iter()
        .map(|param| SignatureParam::new(param_text(param)))
        .collect();
    let labels: Vec<&str> = params.iter().map(|param| param.label.as_str()).collect();
    let label = format!("event {name}({})", labels.join(", "));
    SignatureData::new(label, params, active)
}

/// Params parsed from a builtin detail such as
/// `function prank(address msgSender)`. Forge details stay flat,
/// so a comma split between the outer parens is enough.
fn builtin_signature(builtin: &forge::ForgeBuiltin, active: u32) -> SignatureData {
    SignatureData::new(
        builtin.detail.to_string(),
        detail_params(builtin.detail),
        active,
    )
}

fn detail_params(detail: &str) -> Vec<SignatureParam> {
    let Some(open) = detail.find('(') else {
        return Vec::new();
    };
    let Some(close) = detail.rfind(')') else {
        return Vec::new();
    };
    if close <= open + 1 {
        return Vec::new();
    }
    let Some(inner) = detail.get(open + 1..close) else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(|part| SignatureParam::new(part.trim().to_string()))
        .filter(|param| !param.label.is_empty())
        .collect()
}

fn push_contract_functions(
    contract: &ContractDef,
    name: &str,
    active: u32,
    out: &mut Vec<SignatureData>,
) {
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name != name {
            continue;
        }
        let item = function_signature(&function, active);
        if out.iter().any(|known| known.label == item.label) {
            continue;
        }
        out.push(item);
    }
}

fn push_matching_events(
    events: Vec<EventDef>,
    name: &str,
    active: u32,
    out: &mut Vec<SignatureData>,
) {
    for item in events {
        let Some(item_name) = item.name() else {
            continue;
        };
        if item_name != name {
            continue;
        }
        let entry = event_signature(&item, active);
        if out.iter().any(|known| known.label == entry.label) {
            continue;
        }
        out.push(entry);
    }
}

fn push_contract_getters(
    contract: &ContractDef,
    name: &str,
    active: u32,
    out: &mut Vec<SignatureData>,
) {
    for getter in super::getter::contract_getters(contract) {
        if getter.name != name {
            continue;
        }
        let params: Vec<SignatureParam> = getter
            .params
            .iter()
            .map(|label| SignatureParam::new(label.clone()))
            .collect();
        let label = format!(
            "function {}({}) returns ({})",
            getter.name,
            getter.params.join(", "),
            getter.returns
        );
        if out.iter().any(|known| known.label == label) {
            continue;
        }
        out.push(SignatureData::new(label, params, active));
    }
}

/// Plain call signatures: own contract overloads first, then same
/// file bases in linear order. Shadowed names stay silent. `Test`
/// asserts follow via the forge table, then dep contracts in index
/// order for parity with hover.
fn plain_signatures(
    file: &SolidityFile,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
    offset: u32,
    name: &str,
    active: u32,
) -> Option<Vec<SignatureData>> {
    if let Some(symbol) = scope.resolve(name, offset) {
        if symbol.kind != SymbolKind::Function && symbol.kind != SymbolKind::StateVar {
            return None;
        }
        let mut out = Vec::new();
        if let Some(contract) = contract_at(file, offset) {
            push_contract_functions(&contract, name, active, &mut out);
            push_contract_getters(&contract, name, active, &mut out);
            if let Some(contract_name) = contract.name()
                && !contract_name.is_empty()
            {
                for base in inheritance::linearize(file, &contract_name) {
                    push_contract_functions(&base, name, active, &mut out);
                }
            }
        }
        if out.is_empty() {
            return None;
        }
        return Some(out);
    }
    if let Some(builtin) = forge::plain_builtin(file, scope, offset, name) {
        return Some(vec![builtin_signature(builtin, active)]);
    }
    for (_, dep_file) in deps {
        let mut out = Vec::new();
        for contract in dep_file.contracts() {
            push_contract_functions(&contract, name, active, &mut out);
            if let Some(contract_name) = contract.name() {
                if contract_name.is_empty() {
                    continue;
                }
                for base in inheritance::linearize(dep_file, &contract_name) {
                    push_contract_functions(&base, name, active, &mut out);
                }
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Member call signatures: forge cheatcodes first, then the receiver
/// contract lineage across files. Structs and unknown types stay
/// silent, the same stance as member arity.
fn member_signatures(
    file: &SolidityFile,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
    offset: u32,
    receiver: &str,
    name: &str,
    active: u32,
) -> Option<Vec<SignatureData>> {
    if receiver.is_empty() || name.is_empty() {
        return None;
    }
    if let Some(builtin) = forge::member_builtin(receiver, name, scope, offset) {
        return Some(vec![builtin_signature(builtin, active)]);
    }
    if let Some((info, _)) =
        super::using::resolve_using_member(file, scope, offset, receiver, name, deps)
    {
        let params = detail_params(&info.detail);
        return Some(vec![SignatureData::new(info.detail, params, active)]);
    }
    let kind = members::receiver_type_with_deps(file, scope, offset, receiver, deps);
    if let members::TypeKind::Super { enclosing } = kind {
        if enclosing.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        for base in inheritance::linearize(file, &enclosing) {
            push_contract_functions(&base, name, active, &mut out);
        }
        for (base, _) in inheritance::linearize_across(file, &enclosing, deps) {
            push_contract_functions(&base, name, active, &mut out);
        }
        if out.is_empty() {
            return None;
        }
        return Some(out);
    }
    if matches!(kind, members::TypeKind::UserType(_)) {
        if name != "wrap" && name != "unwrap" {
            return None;
        }
        let (def, _) = inheritance::find_user_type_across(receiver, file, deps)?;
        let type_name = def.name().unwrap_or_default();
        let underlying = def.underlying().map(|node| node.text()).unwrap_or_default();
        if type_name.is_empty() || underlying.is_empty() {
            return None;
        }
        let label = if name == "wrap" {
            format!("function wrap({underlying} value) returns ({type_name})")
        } else {
            format!("function unwrap({type_name} value) returns ({underlying})")
        };
        let params = detail_params(&label);
        return Some(vec![SignatureData::new(label, params, active)]);
    }
    let members::TypeKind::Contract(target) = kind else {
        return None;
    };
    if target.is_empty() {
        return None;
    }
    let (def, _) = inheritance::find_contract_across(&target, file, deps)?;
    let mut out = Vec::new();
    push_contract_functions(&def, name, active, &mut out);
    push_contract_getters(&def, name, active, &mut out);
    for base in inheritance::linearize(file, &target) {
        push_contract_functions(&base, name, active, &mut out);
    }
    for (base, _) in inheritance::linearize_across(file, &target, deps) {
        push_contract_functions(&base, name, active, &mut out);
    }
    if out.is_empty() {
        if let Some((info, _)) =
            super::using::resolve_using_member(file, scope, offset, receiver, name, deps)
        {
            let params = detail_params(&info.detail);
            return Some(vec![SignatureData::new(info.detail, params, active)]);
        }
        return None;
    }
    Some(out)
}

/// Emit signatures from event params: own contract, same file bases,
/// file level, then dep files in index order.
fn event_signatures(
    file: &SolidityFile,
    deps: &[(PathBuf, SolidityFile)],
    offset: u32,
    name: &str,
    active: u32,
) -> Option<Vec<SignatureData>> {
    if name.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    if let Some(contract) = contract_at(file, offset) {
        push_matching_events(contract.events(), name, active, &mut out);
        if let Some(contract_name) = contract.name()
            && !contract_name.is_empty()
        {
            for base in inheritance::linearize(file, &contract_name) {
                push_matching_events(base.events(), name, active, &mut out);
            }
        }
    }
    push_matching_events(file.events(), name, active, &mut out);
    if !out.is_empty() {
        return Some(out);
    }
    for (_, dep_file) in deps {
        push_matching_events(dep_file.events(), name, active, &mut out);
        for contract in dep_file.contracts() {
            push_matching_events(contract.events(), name, active, &mut out);
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

/// Signature help for the call enclosing `offset`.
///
/// Returns one entry per overload sharing the same active index.
/// Plain calls, member calls, `emit` events, and forge builtins
/// resolve with the same order as hover. Positions outside any call
/// and unknown callees yield `None`.
pub fn signature_at(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    deps: &[(PathBuf, SolidityFile)],
) -> Option<Vec<SignatureData>> {
    let site = find_call(text, offset)?;
    let at = site.anchor;
    if site.is_emit {
        return event_signatures(file, deps, at, &site.name, site.active_param);
    }
    let scope = collect_symbols(file);
    if let Some(receiver) = site.receiver {
        return member_signatures(
            file,
            &scope,
            deps,
            at,
            &receiver,
            &site.name,
            site.active_param,
        );
    }
    plain_signatures(file, &scope, deps, at, &site.name, site.active_param)
}
