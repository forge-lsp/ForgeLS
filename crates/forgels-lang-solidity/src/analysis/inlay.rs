use std::path::PathBuf;

use forgels_core::{InlayHintData, MAX_INLAY_HINTS};
use forgels_protocol::InlayHintKind;
use forgels_syntax::{AstNode, Span};

use super::{Scope, collect_symbols};
use crate::SolidityFile;

const CAP: usize = MAX_INLAY_HINTS;

fn contract_at(file: &SolidityFile, offset: u32) -> Option<crate::ContractDef> {
    for contract in file.contracts() {
        let span = contract.syntax().span;
        if span.contains(offset) || span.end == offset {
            return Some(contract);
        }
    }
    None
}

fn param_names_of(function: &crate::FunctionDef) -> Vec<String> {
    function
        .params()
        .iter()
        .map(|param| param.name().unwrap_or_default())
        .collect()
}

fn push_plain_functions(contract: &crate::ContractDef, name: &str, out: &mut Vec<Vec<String>>) {
    for function in contract.functions() {
        let Some(func_name) = function.name() else {
            continue;
        };
        if func_name != name {
            continue;
        }
        let names = param_names_of(&function);
        if !out.contains(&names) {
            out.push(names);
        }
    }
}

fn plain_param_names(
    file: &SolidityFile,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
    anchor: u32,
    name: &str,
) -> Option<Vec<String>> {
    if name.is_empty() {
        return None;
    }
    let Some(symbol) = scope.resolve(name, anchor) else {
        if super::forge::plain_builtin(file, scope, anchor, name).is_some() {
            return None;
        }
        for (_, dep_file) in deps {
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
                        return Some(first);
                    }
                }
            }
        }
        return None;
    };
    if symbol.kind != super::SymbolKind::Function {
        return None;
    }
    if super::forge::plain_builtin(file, scope, anchor, name).is_some() {
        return None;
    }
    if let Some(contract) = contract_at(file, anchor) {
        let mut out = Vec::new();
        push_plain_functions(&contract, name, &mut out);
        if let Some(contract_name) = contract.name()
            && !contract_name.is_empty()
        {
            for base in super::inheritance::linearize(file, &contract_name) {
                push_plain_functions(&base, name, &mut out);
            }
        }
        if let Some(first) = out.into_iter().next() {
            return Some(first);
        }
        return None;
    }
    None
}

fn member_param_names(
    file: &SolidityFile,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
    anchor: u32,
    chain: &[String],
    name: &str,
) -> Option<Vec<String>> {
    if chain.is_empty() || name.is_empty() {
        return None;
    }
    if chain.len() == 2 && super::forge::member_builtin(&chain[0], name, scope, anchor).is_some() {
        return None;
    }
    let kind = super::members::resolve_chained_kind(file, scope, anchor, chain, deps);
    if let super::members::TypeKind::Super { enclosing } = kind {
        if enclosing.is_empty() {
            return None;
        }
        let mut out: Vec<Vec<String>> = Vec::new();
        for base in super::inheritance::linearize(file, &enclosing) {
            push_plain_functions(&base, name, &mut out);
        }
        for (base, _) in super::inheritance::linearize_across(file, &enclosing, deps) {
            push_plain_functions(&base, name, &mut out);
        }
        return out.into_iter().next();
    }
    let super::members::TypeKind::Contract(target) = kind else {
        return None;
    };
    if target.is_empty() {
        return None;
    }
    let (def, _) = super::inheritance::find_contract_across(&target, file, deps)?;
    let mut out: Vec<Vec<String>> = Vec::new();
    push_plain_functions(&def, name, &mut out);
    for base in super::inheritance::linearize(file, &target) {
        push_plain_functions(&base, name, &mut out);
    }
    for (base, _) in super::inheritance::linearize_across(file, &target, deps) {
        push_plain_functions(&base, name, &mut out);
    }
    out.into_iter().next()
}

fn find_open_paren(text: &str, from: usize, limit: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let end = limit.min(bytes.len());
    let mut index = from.min(end);
    let mut in_line = false;
    let mut in_block = false;
    let mut quote: Option<u8> = None;
    while index < end {
        let byte = bytes[index];
        if let Some(q) = quote {
            if byte == b'\\' && index + 1 < end {
                index += 2;
                continue;
            }
            if byte == q {
                quote = None;
            }
            index += 1;
            continue;
        }
        if in_line {
            if byte == b'\n' {
                in_line = false;
            }
            index += 1;
            continue;
        }
        if in_block {
            if byte == b'*' && index + 1 < end && bytes[index + 1] == b'/' {
                in_block = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && index + 1 < end && bytes[index + 1] == b'/' {
            in_line = true;
            index += 2;
            continue;
        }
        if byte == b'/' && index + 1 < end && bytes[index + 1] == b'*' {
            in_block = true;
            index += 2;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'(' {
            return Some(index);
        }
        if byte == b';' || byte == b'{' || byte == b'}' {
            return None;
        }
        index += 1;
    }
    None
}

fn arg_starts(text: &str, open: usize, limit: usize) -> Vec<u32> {
    let bytes = text.as_bytes();
    let end = limit.min(bytes.len());
    let mut out = Vec::new();
    let mut depth_paren: usize = 0;
    let mut depth_bracket: usize = 0;
    let mut depth_brace: usize = 0;
    let mut index = open + 1;
    let mut arg_start: Option<usize> = None;
    let mut in_line = false;
    let mut in_block = false;
    let mut quote: Option<u8> = None;
    let mut closed = false;
    while index < end {
        let byte = bytes[index];
        if let Some(q) = quote {
            if byte == b'\\' && index + 1 < end {
                index += 2;
                continue;
            }
            if byte == q {
                quote = None;
            }
            index += 1;
            continue;
        }
        if in_line {
            if byte == b'\n' {
                in_line = false;
            }
            index += 1;
            continue;
        }
        if in_block {
            if byte == b'*' && index + 1 < end && bytes[index + 1] == b'/' {
                in_block = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && index + 1 < end && bytes[index + 1] == b'/' {
            in_line = true;
            index += 2;
            continue;
        }
        if byte == b'/' && index + 1 < end && bytes[index + 1] == b'*' {
            in_block = true;
            index += 2;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        match byte {
            b'(' => {
                depth_paren += 1;
                index += 1;
            }
            b')' => {
                if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 {
                    closed = true;
                    break;
                }
                depth_paren = depth_paren.saturating_sub(1);
                index += 1;
            }
            b'[' => {
                depth_bracket += 1;
                index += 1;
            }
            b']' => {
                depth_bracket = depth_bracket.saturating_sub(1);
                index += 1;
            }
            b'{' => {
                depth_brace += 1;
                index += 1;
            }
            b'}' => {
                depth_brace = depth_brace.saturating_sub(1);
                index += 1;
            }
            b',' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                if let Some(start) = arg_start {
                    let trimmed = skip_ws(text, start, index);
                    if trimmed < index {
                        out.push(trimmed as u32);
                    }
                }
                arg_start = None;
                index += 1;
                while index < end && bytes[index].is_ascii_whitespace() {
                    index += 1;
                }
                continue;
            }
            _ => {
                if arg_start.is_none()
                    && depth_paren == 0
                    && depth_bracket == 0
                    && depth_brace == 0
                    && !byte.is_ascii_whitespace()
                {
                    arg_start = Some(index);
                }
                index += 1;
            }
        }
    }
    if closed && let Some(start) = arg_start {
        let trimmed = skip_ws(text, start, index);
        if trimmed < index {
            out.push(trimmed as u32);
        }
    }
    if !closed && out.is_empty() {
        return Vec::new();
    }
    out
}

fn skip_ws(text: &str, start: usize, end: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start.min(end.min(bytes.len()));
    while index < end.min(bytes.len()) && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn already_named(text: &str, arg_start: u32) -> bool {
    let bytes = text.as_bytes();
    let mut index = (arg_start as usize).min(bytes.len());
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    let mut end = index;
    while end < bytes.len()
        && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'$')
    {
        end += 1;
    }
    if end == index || end >= bytes.len() {
        return false;
    }
    let mut cursor = end;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    matches!(bytes.get(cursor), Some(b':'))
}

fn push_hints(
    text: &str,
    range: Span,
    param_names: &[String],
    arg_offsets: &[u32],
    out: &mut Vec<InlayHintData>,
) {
    for (index, arg) in arg_offsets.iter().enumerate() {
        if out.len() >= CAP {
            break;
        }
        let Some(param) = param_names.get(index) else {
            continue;
        };
        if param.is_empty() {
            continue;
        }
        if *arg < range.start || *arg > range.end {
            continue;
        }
        if text.get(*arg as usize..).is_none() {
            continue;
        }
        if already_named(text, *arg) {
            continue;
        }
        let mut label = param.clone();
        label.push(':');
        out.push(InlayHintData::new(*arg, label, InlayHintKind::PARAMETER));
    }
}

/// Parameter name hints for plain and member calls in `range`.
///
/// Resolves plain calls through the lexical scope plus deps and member
/// calls through the receiver contract lineage. Builtins and unknown
/// callees stay silent. Slots are lexical: the first non-ws byte after
/// `(` or `,` at depth zero. Unnamed params and already named args
/// stay silent. Caps at `MAX_INLAY_HINTS`.
pub fn inlay_hints_in(
    file: &SolidityFile,
    text: &str,
    range: Span,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<InlayHintData> {
    let mut out = Vec::new();
    if text.is_empty() {
        return out;
    }
    let scope = collect_symbols(file);
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.all_calls() {
                if out.len() >= CAP {
                    break;
                }
                let Some(callee) = call.callee() else {
                    continue;
                };
                if callee.is_empty() {
                    continue;
                }
                let span = call.syntax().span;
                if span.end < range.start || span.start > range.end {
                    continue;
                }
                let anchor = span.start;
                let Some(names) = plain_param_names(file, &scope, deps, anchor, &callee) else {
                    continue;
                };
                if names.is_empty() {
                    continue;
                }
                let from = span.start as usize;
                let limit = (span.end as usize).max(from);
                let Some(open) = find_open_paren(text, from, limit) else {
                    continue;
                };
                let args = arg_starts(text, open, limit);
                if args.is_empty() {
                    continue;
                }
                push_hints(text, range, &names, &args, &mut out);
            }
            for member in body.all_members() {
                if out.len() >= CAP {
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
                let span = member.syntax().span;
                if span.end < range.start || span.start > range.end {
                    continue;
                }
                let anchor = span.start;
                let Some(names) = member_param_names(file, &scope, deps, anchor, &chain, &name)
                else {
                    continue;
                };
                if names.is_empty() {
                    continue;
                }
                let from = span.start as usize;
                let limit = (span.end as usize).max(from);
                let Some(open) = find_open_paren(text, from, limit) else {
                    continue;
                };
                let args = arg_starts(text, open, limit);
                if args.is_empty() {
                    continue;
                }
                push_hints(text, range, &names, &args, &mut out);
            }
        }
        if out.len() >= CAP {
            break;
        }
    }
    out.sort_by_key(|left| left.offset);
    out.truncate(CAP);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgels_syntax::Lexer;

    fn parse(text: &str) -> SolidityFile {
        let tokens = crate::SolidityLexer.tokenize(text);
        let (file, _) = crate::SolidityParser.parse_with_errors(&tokens);
        file
    }

    fn hints(text: &str) -> Vec<InlayHintData> {
        let file = parse(text);
        inlay_hints_in(&file, text, Span::new(0, text.len() as u32), &[])
    }

    #[test]
    fn plain_call_shows_param_names() {
        let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
        let found = hints(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].label, "a:");
        assert_eq!(found[1].label, "b:");
        assert_eq!(found[0].kind, InlayHintKind::PARAMETER);
    }

    #[test]
    fn member_call_shows_param_names() {
        let text = "contract H {\nfunction greet(uint to, uint amount) public {}\n}\ncontract C {\nfunction f() public {\nH h;\nh.greet(1, 2);\n}\n}";
        let found = hints(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].label, "to:");
        assert_eq!(found[1].label, "amount:");
    }

    #[test]
    fn super_call_shows_base_param_names() {
        let text = "contract B {\nfunction greet(uint to, uint amount) public {}\n}\ncontract D is B {\nfunction f() public {\nsuper.greet(1, 2);\n}\n}";
        let found = hints(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].label, "to:");
        assert_eq!(found[1].label, "amount:");
    }

    #[test]
    fn builtin_and_unknown_stay_silent() {
        let text =
            "contract T is Test {\nfunction f() public {\nassertEq(1, 2);\nmissing(1);\n}\n}";
        assert!(hints(text).is_empty());
    }

    #[test]
    fn range_filters_outside_hints() {
        let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
        let file = parse(text);
        let at = text.find("greet(1").expect("call") as u32;
        let found = inlay_hints_in(&file, text, Span::new(0, at), &[]);
        assert!(found.is_empty());
    }

    #[test]
    fn unnamed_params_stay_silent() {
        let text = "contract C {\nfunction greet(uint, uint b) public {}\nfunction f() public {\ngreet(1, 2);\n}\n}";
        let found = hints(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "b:");
    }

    #[test]
    fn nested_plain_call_shows_param_names_once() {
        let text = "contract C {\nfunction greet(uint a, uint b) public {}\nfunction f() public {\nif (true) {\ngreet(1, 2);\n}\n}\n}";
        let found = hints(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].label, "a:");
        assert_eq!(found[1].label, "b:");
    }

    #[test]
    fn nested_member_call_shows_param_names_once() {
        let text = "contract H {\nfunction greet(uint to, uint amount) public {}\n}\ncontract C {\nfunction f() public {\nH h;\nwhile (true) {\nh.greet(1, 2);\n}\n}\n}";
        let found = hints(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].label, "to:");
        assert_eq!(found[1].label, "amount:");
    }
}
