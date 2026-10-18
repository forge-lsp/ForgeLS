use std::collections::HashSet;
use std::path::PathBuf;

use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::{AstNode, Lexer, Span};

use super::{Scope, collect_symbols, forge};
use crate::lexer::SolidityKind;
use crate::{SolidityFile, SolidityLexer, SolidityNode};

/// Receivers that never error, even with no declaration.
///
/// Covers `msg.sender`, `block.timestamp`, `abi.encode`, `vm.*`,
/// `console.*`, plus `this` and `super`. User declarations shadow
/// them via the scope check first.
pub(crate) const GLOBAL_RECEIVERS: &[&str] = &[
    "msg", "block", "tx", "abi", "vm", "console", "this", "super",
];

/// Plain globals that never error.
///
/// Covers builtin calls, hash helpers, `Error` and `Panic` catch names,
/// and the `Test` base from forge-std which lives outside the
/// workspace. User declarations still win via the scope check first.
pub(crate) const ALWAYS_OK: &[&str] = &[
    "require",
    "assert",
    "revert",
    "keccak256",
    "sha256",
    "ripemd160",
    "ecrecover",
    "addmod",
    "mulmod",
    "selfdestruct",
    "gasleft",
    "blockhash",
    "now",
    "Test",
    "Error",
    "Panic",
];

/// Function type spans: every `Type` node starting with `function`.
///
/// Inner param names and type words stay silent for unknown, references,
/// and rename. The outer variable name lives outside the type node, so
/// it still lints and resolves normally.
pub(crate) fn function_type_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    collect_function_types(file.syntax(), &mut out);
    out
}

fn collect_function_types(node: &forgels_syntax::SyntaxNode, out: &mut Vec<Span>) {
    if node.kind == SolidityNode::Type.as_u16()
        && node.children.first().is_some_and(|element| {
            matches!(element, forgels_syntax::SyntaxElement::Token(token) if token.text == "function")
        })
    {
        out.push(node.span);
    }
    for element in &node.children {
        if let forgels_syntax::SyntaxElement::Node(child) = element {
            collect_function_types(child, out);
        }
    }
}

/// Unknown identifier errors: every `Ident` token must resolve.
///
/// Resolve order is scope, dep exports, forge asserts, allowlists,
/// then one way prefix escape for typing state. Member names after
/// a dot stay silent so member arity owns them. Emit, revert, and
/// new names stay silent so their owners report without doubles.
/// Assembly idents use Yul resolve first: Yul decl, Yul builtin, then
/// outer scope. Remaining assembly idents stay silent: unknown opcodes
/// are deferred, so they never error here.
pub fn unknown_identifier_diagnostics(
    file: &SolidityFile,
    text: &str,
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
) -> Vec<DiagnosticData> {
    let tokens = SolidityLexer.tokenize(text);
    let decls = decl_spans(file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let imports = import_spans(file);
    let emits = emit_spans(file);
    let emit_set: HashSet<(u32, u32)> = emits.iter().map(|span| (span.start, span.end)).collect();
    let reverts = revert_spans(file);
    let revert_set: HashSet<(u32, u32)> =
        reverts.iter().map(|span| (span.start, span.end)).collect();
    let news = new_spans(file);
    let new_set: HashSet<(u32, u32)> = news.iter().map(|span| (span.start, span.end)).collect();
    let members = member_spans(file);
    let member_set: HashSet<(u32, u32)> =
        members.iter().map(|span| (span.start, span.end)).collect();
    let options = options_spans(file);
    let assemblies = assembly_ranges(&tokens);
    let yul_decls = super::yul::collect_yul_decls(&tokens, &assemblies);
    let usings = super::using::using_spans(file);
    let func_types = function_type_spans(file);
    let dep_names = dep_export_names(deps);
    let mut out = Vec::new();
    for token in &tokens {
        if token.kind != SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text.is_empty() {
            continue;
        }
        let span = token.span;
        let name = token.text.as_str();
        if inside_any(&assemblies, span.start) {
            if super::yul::is_yul_decl_token(&yul_decls, span) {
                continue;
            }
            if super::yul::is_yul_builtin(name) {
                continue;
            }
            if super::yul::find_yul_decl(&yul_decls, name, span.start).is_some() {
                continue;
            }
            if scope.resolve(name, span.start).is_some() {
                continue;
            }
            if dep_names.contains(name) {
                continue;
            }
            continue;
        }
        if decl_set.contains(&(span.start, span.end)) {
            continue;
        }
        if inside_any(&imports, span.start) {
            continue;
        }
        if emit_set.contains(&(span.start, span.end)) {
            continue;
        }
        if revert_set.contains(&(span.start, span.end)) {
            continue;
        }
        if new_set.contains(&(span.start, span.end)) {
            continue;
        }
        if member_set.contains(&(span.start, span.end)) {
            continue;
        }
        if inside_any(&options, span.start) {
            continue;
        }
        if inside_any(&usings, span.start) {
            continue;
        }
        if inside_any(&func_types, span.start) {
            continue;
        }
        if preceded_by_dot(text, span.start) {
            continue;
        }
        if name == "_" {
            continue;
        }
        if scope.resolve(name, span.start).is_some() {
            continue;
        }
        if dep_names.contains(name) {
            continue;
        }
        if forge::plain_builtin(file, scope, span.start, name).is_some() {
            continue;
        }
        if GLOBAL_RECEIVERS.contains(&name) || ALWAYS_OK.contains(&name) {
            continue;
        }
        if is_typing_prefix(scope, deps, name, span.start) {
            continue;
        }
        out.push(DiagnosticData::error(
            span,
            format!("unresolved identifier '{name}'"),
            RuleId::UNRESOLVED_IDENTIFIER,
        ));
    }
    out
}

pub(crate) fn inside_any(spans: &[Span], offset: u32) -> bool {
    spans.iter().any(|span| span.contains(offset))
}

/// True when the byte before `start` is a dot with no gap.
///
/// Covers chained `a.b.c` tails the single level AST drops. Receiver
/// side stays checked: only the name after the dot escapes.
pub(crate) fn preceded_by_dot(text: &str, start: u32) -> bool {
    let index = start as usize;
    if index == 0 {
        return false;
    }
    matches!(text.as_bytes().get(index - 1), Some(b'.'))
}

/// Every declaration name span: contracts, functions, params, vars,
/// structs plus fields, enums plus variants, errors, user types,
/// events, modifiers.
pub(crate) fn decl_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for item in file.structs() {
        if let Some(span) = item.name_span() {
            out.push(span);
        }
        for field in item.fields() {
            if let Some(span) = field.name_span() {
                out.push(span);
            }
        }
    }
    for item in file.enums() {
        if let Some(span) = item.name_span() {
            out.push(span);
        }
        out.extend(item.variant_spans());
    }
    for item in file.errors() {
        if let Some(span) = item.name_span() {
            out.push(span);
        }
        for param in item.params() {
            if let Some(span) = param.name_span() {
                out.push(span);
            }
        }
    }
    for item in file.user_types() {
        if let Some(span) = item.name_span() {
            out.push(span);
        }
    }
    for item in file.events() {
        if let Some(span) = item.name_span() {
            out.push(span);
        }
        for param in item.params() {
            if let Some(span) = param.name_span() {
                out.push(span);
            }
        }
    }
    for contract in file.contracts() {
        if let Some(span) = contract.name_span() {
            out.push(span);
        }
        for item in contract.structs() {
            if let Some(span) = item.name_span() {
                out.push(span);
            }
            for field in item.fields() {
                if let Some(span) = field.name_span() {
                    out.push(span);
                }
            }
        }
        for item in contract.enums() {
            if let Some(span) = item.name_span() {
                out.push(span);
            }
            out.extend(item.variant_spans());
        }
        for item in contract.errors() {
            if let Some(span) = item.name_span() {
                out.push(span);
            }
            for param in item.params() {
                if let Some(span) = param.name_span() {
                    out.push(span);
                }
            }
        }
        for item in contract.user_types() {
            if let Some(span) = item.name_span() {
                out.push(span);
            }
        }
        for item in contract.events() {
            if let Some(span) = item.name_span() {
                out.push(span);
            }
            for param in item.params() {
                if let Some(span) = param.name_span() {
                    out.push(span);
                }
            }
        }
        for item in contract.modifiers() {
            if let Some(span) = item.name_span() {
                out.push(span);
            }
        }
        for var in contract.state_vars() {
            if let Some(span) = var.name_span() {
                out.push(span);
            }
        }
        for function in contract.functions() {
            if let Some(span) = function.name_span() {
                out.push(span);
            }
            for param in function.params() {
                if let Some(span) = param.name_span() {
                    out.push(span);
                }
            }
            if let Some(body) = function.body() {
                for local in body.all_locals() {
                    if let Some(span) = local.name_span() {
                        out.push(span);
                    }
                }
                for param in body.all_params() {
                    if let Some(span) = param.name_span() {
                        out.push(span);
                    }
                }
            }
        }
    }
    out
}

/// Whole import and pragma spans: any ident inside is metadata.
pub(crate) fn import_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for item in file.imports() {
        out.push(item.syntax().span);
    }
    for item in file.pragmas() {
        out.push(item.syntax().span);
    }
    out
}

/// Emit name spans: owned by `unknown_emit`, never double here.
pub(crate) fn emit_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for emit in body.all_emits() {
                if let Some(span) = emit.name_span() {
                    out.push(span);
                }
            }
        }
    }
    out
}

/// Revert name spans: owned by `unknown_error`, never double here.
pub(crate) fn revert_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for revert in body.all_reverts() {
                if let Some(span) = revert.name_span() {
                    out.push(span);
                }
            }
        }
    }
    out
}

/// Deploy name spans: deploy arity owns them, never double here.
pub(crate) fn new_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for deploy in body.all_news() {
                if let Some(span) = deploy.name_span() {
                    out.push(span);
                }
            }
        }
    }
    out
}

/// Member name spans after the dot: arity and hover own them.
///
/// Covers every tail in a chain: `a.b.c` yields both `b` and `c` so
/// intermediates stay silent and only the final segment resolves.
pub(crate) fn member_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for member in body.all_members() {
                out.extend(member.all_member_spans());
            }
        }
    }
    out
}

/// Call option spans `{...}`: option names like `value` never lint.
pub(crate) fn options_spans(file: &SolidityFile) -> Vec<Span> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.all_calls() {
                if let Some(span) = call.options_span() {
                    out.push(span);
                }
            }
            for member in body.all_members() {
                if let Some(span) = member.options_span() {
                    out.push(span);
                }
            }
            for deploy in body.all_news() {
                if let Some(span) = deploy.options_span() {
                    out.push(span);
                }
            }
        }
    }
    out
}

/// Assembly body ranges from tokens: dialect string and braces.
///
/// The parser skips assembly without nodes, so rebuild ranges here
/// with balanced brace counting. Idents inside never lint.
pub(crate) fn assembly_ranges(tokens: &[forgels_syntax::Token]) -> Vec<Span> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index].kind != SolidityKind::Assembly.as_u16() {
            index += 1;
            continue;
        }
        let start = tokens[index].span.start;
        let mut cursor = index + 1;
        if cursor < tokens.len() && tokens[cursor].kind == SolidityKind::StringLit.as_u16() {
            cursor += 1;
        }
        if cursor < tokens.len() && tokens[cursor].kind == SolidityKind::LParen.as_u16() {
            let mut depth = 0u32;
            while cursor < tokens.len() {
                if tokens[cursor].kind == SolidityKind::LParen.as_u16() {
                    depth += 1;
                } else if tokens[cursor].kind == SolidityKind::RParen.as_u16() {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    if depth == 0 {
                        cursor += 1;
                        break;
                    }
                }
                cursor += 1;
            }
        }
        if cursor >= tokens.len() || tokens[cursor].kind != SolidityKind::LBrace.as_u16() {
            index = cursor.max(index + 1);
            continue;
        }
        let mut depth = 0u32;
        let mut end = tokens[cursor].span.end;
        while cursor < tokens.len() {
            if tokens[cursor].kind == SolidityKind::LBrace.as_u16() {
                depth += 1;
            } else if tokens[cursor].kind == SolidityKind::RBrace.as_u16() {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                end = tokens[cursor].span.end;
                if depth == 0 {
                    cursor += 1;
                    break;
                }
            }
            cursor += 1;
        }
        out.push(Span::new(start, end));
        index = cursor.max(index + 1);
    }
    out
}

/// Exported dep names for the scope miss fallback.
fn dep_export_names(deps: &[(PathBuf, SolidityFile)]) -> HashSet<String> {
    let mut out = HashSet::new();
    for (_, dep_file) in deps {
        let scope = collect_symbols(dep_file);
        for symbol in scope.export_symbols() {
            out.insert(symbol.name.clone());
        }
    }
    out
}

/// One way prefix escape for typing: unknown is a strict prefix.
///
/// `tot` before `total` stays silent while typing. `totalX` past a
/// known name still errors. Checks visible scope, dep exports, forge
/// tables, and allowlists in one pass.
fn is_typing_prefix(
    scope: &Scope,
    deps: &[(PathBuf, SolidityFile)],
    name: &str,
    offset: u32,
) -> bool {
    if name.is_empty() {
        return false;
    }
    for symbol in scope.visible_at(offset) {
        if symbol.name.len() > name.len() && symbol.name.starts_with(name) {
            return true;
        }
    }
    for (_, dep_file) in deps {
        let dep_scope = collect_symbols(dep_file);
        for symbol in dep_scope.export_symbols() {
            if symbol.name.len() > name.len() && symbol.name.starts_with(name) {
                return true;
            }
        }
    }
    for builtin in forge::ASSERT_TABLE
        .iter()
        .chain(forge::VM_TABLE.iter())
        .chain(forge::CONSOLE_TABLE.iter())
    {
        if builtin.name.len() > name.len() && builtin.name.starts_with(name) {
            return true;
        }
    }
    for known in GLOBAL_RECEIVERS.iter().chain(ALWAYS_OK.iter()) {
        if known.len() > name.len() && known.starts_with(name) {
            return true;
        }
    }
    false
}
