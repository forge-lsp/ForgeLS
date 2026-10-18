use forgels_syntax::{Span, Token};

use crate::lexer::SolidityKind;

/// Origin tag for Yul builtins in completion and hover details.
pub(crate) const YUL_ORIGIN: &str = "yul";

/// Yul opcode subset used for completion, hover, and silent checks.
///
/// Covers arithmetic, logic, hashing, environment, memory, storage,
/// logs, and control calls. Anything uncertain stays out and silent:
/// unknown opcodes never error (see docs, unknown-opcode deferred).
pub(crate) const YUL_BUILTINS: &[&str] = &[
    "stop",
    "add",
    "sub",
    "mul",
    "div",
    "sdiv",
    "mod",
    "smod",
    "exp",
    "not",
    "lt",
    "gt",
    "slt",
    "sgt",
    "eq",
    "iszero",
    "and",
    "or",
    "xor",
    "byte",
    "shl",
    "shr",
    "sar",
    "addmod",
    "mulmod",
    "signextend",
    "keccak256",
    "address",
    "balance",
    "origin",
    "caller",
    "callvalue",
    "calldataload",
    "calldatasize",
    "calldatacopy",
    "codesize",
    "codecopy",
    "gasprice",
    "extcodesize",
    "extcodecopy",
    "returndatasize",
    "returndatacopy",
    "extcodehash",
    "blockhash",
    "coinbase",
    "timestamp",
    "number",
    "difficulty",
    "prevrandao",
    "gaslimit",
    "chainid",
    "selfbalance",
    "basefee",
    "blobhash",
    "blobbasefee",
    "pop",
    "mload",
    "mstore",
    "mstore8",
    "sload",
    "sstore",
    "msize",
    "gas",
    "log0",
    "log1",
    "log2",
    "log3",
    "log4",
    "create",
    "create2",
    "call",
    "callcode",
    "delegatecall",
    "staticcall",
    "return",
    "revert",
    "invalid",
    "selfdestruct",
];

/// Yul declaration sort: `let` bindings, function names, function params.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum YulKind {
    Let,
    Function,
    Param,
}

/// One flat Yul declaration: name plus its token span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YulDecl {
    pub name: String,
    pub span: Span,
    pub kind: YulKind,
}

/// True for Yul opcode names. Empty names never match.
pub(crate) fn is_yul_builtin(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    YUL_BUILTINS.contains(&name)
}

/// Display detail for a Yul builtin, for example `function add - from yul`.
pub(crate) fn yul_builtin_detail(name: &str) -> String {
    format!("function {name} - from {YUL_ORIGIN}")
}

/// Display detail for a Yul declaration.
pub(crate) fn yul_decl_detail(decl: &YulDecl) -> String {
    match decl.kind {
        YulKind::Function => format!("function {} - from {YUL_ORIGIN}", decl.name),
        YulKind::Let | YulKind::Param => format!("let {} - from {YUL_ORIGIN}", decl.name),
    }
}

/// True when `offset` sits inside any assembly range.
pub(crate) fn is_in_assembly(assemblies: &[Span], offset: u32) -> bool {
    assemblies.iter().any(|span| span.contains(offset))
}

/// True when `span` matches a collected Yul declaration span.
pub(crate) fn is_yul_decl_token(decls: &[YulDecl], span: Span) -> bool {
    decls
        .iter()
        .any(|decl| decl.span.start == span.start && decl.span.end == span.end)
}

/// Latest Yul declaration for `name` visible at `offset`.
///
/// Flat and order checked: only decls with `start <= offset` count, the
/// latest wins. Nested shadowing stays imprecise by design (deferred).
pub(crate) fn find_yul_decl<'a>(
    decls: &'a [YulDecl],
    name: &str,
    offset: u32,
) -> Option<&'a YulDecl> {
    if name.is_empty() {
        return None;
    }
    let mut best: Option<&YulDecl> = None;
    for decl in decls {
        if decl.name != name {
            continue;
        }
        if decl.span.start > offset {
            continue;
        }
        if best.is_none_or(|prev| prev.span.start <= decl.span.start) {
            best = Some(decl);
        }
    }
    best
}

fn is_comment(kind: u16) -> bool {
    kind == SolidityKind::Comment.as_u16()
}

fn skip_comments(tokens: &[Token], mut cursor: usize) -> usize {
    while cursor < tokens.len() && is_comment(tokens[cursor].kind) {
        cursor += 1;
    }
    cursor
}

fn inside_any(assemblies: &[Span], offset: u32) -> bool {
    assemblies.iter().any(|span| span.contains(offset))
}

/// Flat Yul declarations inside `assemblies` in token order.
///
/// Collects `let x, y` names and `function f(params) -> rets` names plus
/// params. Assignment uses (`x := ...`) never count as decls. Only
/// tokens inside `assemblies` count, so Solidity `function` rows stay out.
pub(crate) fn collect_yul_decls(tokens: &[Token], assemblies: &[Span]) -> Vec<YulDecl> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if is_comment(token.kind) {
            index += 1;
            continue;
        }
        if !inside_any(assemblies, token.span.start) {
            index += 1;
            continue;
        }
        if token.kind == SolidityKind::Reserved.as_u16() && token.text == "let" {
            index = collect_let(tokens, index, &mut out);
            continue;
        }
        if token.kind == SolidityKind::Function.as_u16() {
            index = collect_function(tokens, index, &mut out);
            continue;
        }
        index += 1;
    }
    out
}

fn collect_let(tokens: &[Token], at: usize, out: &mut Vec<YulDecl>) -> usize {
    let mut cursor = skip_comments(tokens, at + 1);
    loop {
        let Some(token) = tokens.get(cursor) else {
            return cursor;
        };
        if is_comment(token.kind) {
            cursor += 1;
            continue;
        }
        if token.kind != SolidityKind::Ident.as_u16() || token.text.is_empty() {
            return cursor;
        }
        out.push(YulDecl {
            name: token.text.clone(),
            span: token.span,
            kind: YulKind::Let,
        });
        cursor = skip_comments(tokens, cursor + 1);
        let Some(next) = tokens.get(cursor) else {
            return cursor;
        };
        if next.kind != SolidityKind::Comma.as_u16() {
            return cursor;
        }
        cursor = skip_comments(tokens, cursor + 1);
    }
}

fn collect_function(tokens: &[Token], at: usize, out: &mut Vec<YulDecl>) -> usize {
    let mut cursor = skip_comments(tokens, at + 1);
    let Some(name) = tokens.get(cursor) else {
        return cursor;
    };
    if name.kind != SolidityKind::Ident.as_u16() || name.text.is_empty() {
        return cursor;
    }
    out.push(YulDecl {
        name: name.text.clone(),
        span: name.span,
        kind: YulKind::Function,
    });
    cursor = skip_comments(tokens, cursor + 1);
    let Some(open) = tokens.get(cursor) else {
        return cursor;
    };
    if open.kind != SolidityKind::LParen.as_u16() {
        return cursor;
    }
    cursor = skip_comments(tokens, cursor + 1);
    loop {
        let Some(token) = tokens.get(cursor) else {
            return cursor;
        };
        if is_comment(token.kind) {
            cursor += 1;
            continue;
        }
        if token.kind == SolidityKind::RParen.as_u16() {
            cursor += 1;
            break;
        }
        if token.kind == SolidityKind::Ident.as_u16() && !token.text.is_empty() {
            out.push(YulDecl {
                name: token.text.clone(),
                span: token.span,
                kind: YulKind::Param,
            });
            cursor = skip_comments(tokens, cursor + 1);
            let Some(next) = tokens.get(cursor) else {
                return cursor;
            };
            if next.kind == SolidityKind::Comma.as_u16() {
                cursor = skip_comments(tokens, cursor + 1);
                continue;
            }
            continue;
        }
        if token.kind == SolidityKind::Comma.as_u16() {
            cursor += 1;
            continue;
        }
        cursor += 1;
    }
    cursor = skip_comments(tokens, cursor);
    let Some(arrow) = tokens.get(cursor) else {
        return cursor;
    };
    if arrow.kind != SolidityKind::RightArrow.as_u16() {
        return cursor;
    }
    cursor = skip_comments(tokens, cursor + 1);
    loop {
        let Some(token) = tokens.get(cursor) else {
            return cursor;
        };
        if is_comment(token.kind) {
            cursor += 1;
            continue;
        }
        if token.kind == SolidityKind::Ident.as_u16() && !token.text.is_empty() {
            out.push(YulDecl {
                name: token.text.clone(),
                span: token.span,
                kind: YulKind::Param,
            });
            cursor = skip_comments(tokens, cursor + 1);
            let Some(next) = tokens.get(cursor) else {
                return cursor;
            };
            if next.kind == SolidityKind::Comma.as_u16() {
                cursor = skip_comments(tokens, cursor + 1);
                continue;
            }
            return cursor;
        }
        return cursor;
    }
}
