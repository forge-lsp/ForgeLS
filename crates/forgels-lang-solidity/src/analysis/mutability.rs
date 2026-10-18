use std::collections::HashSet;

use forgels_core::DiagnosticData;
use forgels_syntax::{AstNode, Lexer};

use super::{Scope, SymbolKind, unknown};
use crate::{SolidityFile, SolidityLexer, lexer::SolidityKind};

fn is_write_kind(kind: u16) -> bool {
    kind == SolidityKind::Assign.as_u16()
        || kind == SolidityKind::PlusAssign.as_u16()
        || kind == SolidityKind::MinusAssign.as_u16()
        || kind == SolidityKind::StarAssign.as_u16()
        || kind == SolidityKind::SlashAssign.as_u16()
        || kind == SolidityKind::PercentAssign.as_u16()
        || kind == SolidityKind::AmpAssign.as_u16()
        || kind == SolidityKind::PipeAssign.as_u16()
        || kind == SolidityKind::CaretAssign.as_u16()
        || kind == SolidityKind::ShlAssign.as_u16()
        || kind == SolidityKind::AssignSar.as_u16()
        || kind == SolidityKind::AssignShr.as_u16()
        || kind == SolidityKind::PlusPlus.as_u16()
        || kind == SolidityKind::MinusMinus.as_u16()
}

fn receiver_before(text: &str, dot: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if dot == 0 || dot > bytes.len() {
        return None;
    }
    let mut start = dot;
    while start > 0
        && (bytes[start - 1].is_ascii_alphanumeric()
            || bytes[start - 1] == b'_'
            || bytes[start - 1] == b'$')
    {
        start -= 1;
    }
    if start == dot {
        return None;
    }
    text.get(start..dot).map(|part| part.to_string())
}

/// Pure and view state access checks via token scan plus member writes.
///
/// Every `Ident` in the body resolves through the scope; only `StateVar`
/// hits count. Declaration names, imports, and assembly bodies are
/// skipped. `this.x` counts as a read of `x`; `delete`, any assignment,
/// and `++` or `--` around the usage count as a write. Pure errors on
/// reads and writes, view errors on writes only. Member writes cover
/// `arr.push` and `arr.pop` plus direct `s.field` on the left of an
/// assign family with a `StateVar` root via `all_members`. Longer
/// chains, mappings, assembly, cross-function, and call targets stay
/// silent by design.
pub fn mutability_diagnostics(
    file: &SolidityFile,
    text: &str,
    scope: &Scope,
) -> Vec<DiagnosticData> {
    let tokens = SolidityLexer.tokenize(text);
    let decls = unknown::decl_spans(file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let imports = unknown::import_spans(file);
    let assemblies = unknown::assembly_ranges(&tokens);
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(func_name) = function.name() else {
                continue;
            };
            if func_name.is_empty() {
                continue;
            }
            let Some(mutability) = function.mutability() else {
                continue;
            };
            let is_pure = mutability == crate::Mutability::Pure;
            let is_view = mutability == crate::Mutability::View;
            if !is_pure && !is_view {
                continue;
            }
            let Some(body) = function.body() else {
                continue;
            };
            let body_span = body.syntax().span;
            for (index, token) in tokens.iter().enumerate() {
                if token.kind != SolidityKind::Ident.as_u16() {
                    continue;
                }
                if token.text.is_empty() {
                    continue;
                }
                let span = token.span;
                if !body_span.contains(span.start) {
                    continue;
                }
                if decl_set.contains(&(span.start, span.end)) {
                    continue;
                }
                if unknown::inside_any(&imports, span.start) {
                    continue;
                }
                if unknown::inside_any(&assemblies, span.start) {
                    continue;
                }
                let name = token.text.as_str();
                let after_dot = unknown::preceded_by_dot(text, span.start);
                if after_dot {
                    let dot = span.start as usize - 1;
                    let Some(receiver) = receiver_before(text, dot) else {
                        continue;
                    };
                    if receiver != "this" {
                        continue;
                    }
                }
                let Some(symbol) = scope.resolve(name, span.start) else {
                    continue;
                };
                if symbol.kind != SymbolKind::StateVar {
                    continue;
                }
                // Member writes own their root: `arr.push` and `s.x =`
                // report once via `all_members`, so the token read here
                // stays silent and pure shows one write, not read+write.
                if !after_dot && root_has_member_write(&tokens, index) {
                    continue;
                }
                let prev = prev_code(&tokens, index);
                let next = next_code_token(&tokens, index);
                let mut is_write = false;
                if let Some(before) = prev
                    && (before.kind == SolidityKind::Delete.as_u16()
                        || before.kind == SolidityKind::PlusPlus.as_u16()
                        || before.kind == SolidityKind::MinusMinus.as_u16())
                {
                    is_write = true;
                }
                if let Some(after) = next
                    && is_write_kind(after.kind)
                {
                    is_write = true;
                }
                // `delete this.x`: the `delete` sits before `this`, not
                // before `x`. Walk back over `this` and the dot.
                if after_dot && !is_write {
                    is_write = delete_before_this(&tokens, index);
                }
                if is_pure {
                    if is_write {
                        out.push(DiagnosticData {
                            span,
                            message: format!(
                                "pure function '{func_name}' cannot write to state variable '{name}'"
                            ),
                        });
                    } else {
                        out.push(DiagnosticData {
                            span,
                            message: format!(
                                "pure function '{func_name}' cannot read state variable '{name}'"
                            ),
                        });
                    }
                } else if is_write {
                    out.push(DiagnosticData {
                        span,
                        message: format!(
                            "view function '{func_name}' cannot write to state variable '{name}'"
                        ),
                    });
                }
            }
            for member in body.all_members() {
                check_member_write(
                    &mut out,
                    &tokens,
                    &assemblies,
                    scope,
                    &member,
                    func_name.as_str(),
                    is_pure,
                    is_view,
                );
            }
        }
    }
    out
}

fn prev_code(tokens: &[forgels_syntax::Token], index: usize) -> Option<forgels_syntax::Token> {
    let comment = SolidityKind::Comment.as_u16();
    let mut at = index.checked_sub(1)?;
    loop {
        let token = tokens.get(at)?;
        if token.kind != comment {
            return Some(token.clone());
        }
        at = at.checked_sub(1)?;
    }
}

fn next_code_token(
    tokens: &[forgels_syntax::Token],
    index: usize,
) -> Option<forgels_syntax::Token> {
    let comment = SolidityKind::Comment.as_u16();
    let mut at = index + 1;
    loop {
        let token = tokens.get(at)?;
        if token.kind != comment {
            return Some(token.clone());
        }
        at += 1;
    }
}

/// True when `delete` precedes `this` in `delete this.x`.
fn delete_before_this(tokens: &[forgels_syntax::Token], x_index: usize) -> bool {
    let comment = SolidityKind::Comment.as_u16();
    // x <- dot <- this <- maybe delete. Walk back three code tokens.
    let mut codes: Vec<forgels_syntax::Token> = Vec::new();
    let mut at = x_index.checked_sub(1);
    while codes.len() < 3 {
        let Some(pos) = at else {
            break;
        };
        let Some(token) = tokens.get(pos) else {
            break;
        };
        if token.kind == comment {
            at = pos.checked_sub(1);
            continue;
        }
        codes.push(token.clone());
        at = pos.checked_sub(1);
    }
    if codes.len() < 3 {
        return false;
    }
    if codes[0].kind != SolidityKind::Dot.as_u16() {
        return false;
    }
    if codes[1].text != "this" {
        return false;
    }
    codes[2].kind == SolidityKind::Delete.as_u16()
        || codes[2].kind == SolidityKind::PlusPlus.as_u16()
        || codes[2].kind == SolidityKind::MinusMinus.as_u16()
}

/// Member writes for view and pure: `arr.push`, `arr.pop`, and
/// direct `s.field` on the left of an assign family.
///
/// Only two-segment chains count with a `StateVar` root. Assembly
/// stays silent. Reads stay silent here: the token scan above owns
/// them. Mappings, longer chains, and call targets stay silent.
#[allow(clippy::too_many_arguments)]
fn check_member_write(
    out: &mut Vec<DiagnosticData>,
    tokens: &[forgels_syntax::Token],
    assemblies: &[forgels_syntax::Span],
    scope: &Scope,
    member: &crate::MemberExpr,
    func_name: &str,
    is_pure: bool,
    is_view: bool,
) {
    if func_name.is_empty() {
        return;
    }
    let chain = member.chain();
    if chain.len() != 2 {
        return;
    }
    let root = chain.first().cloned().unwrap_or_default();
    if root.is_empty() || root == "this" || root == "super" {
        return;
    }
    let Some(field) = member.final_member() else {
        return;
    };
    if field.is_empty() {
        return;
    }
    let span = member.syntax().span;
    if unknown::inside_any(assemblies, span.start) {
        return;
    }
    let offset = span.start;
    let Some(symbol) = scope.resolve(&root, offset) else {
        return;
    };
    if symbol.kind != SymbolKind::StateVar {
        return;
    }
    let is_push_pop = member.is_call() && (field == "push" || field == "pop");
    let mut is_write = is_push_pop;
    if !is_write && !member.is_call() {
        let Some(next) = next_code_after(tokens, span.end) else {
            return;
        };
        if is_write_kind(next.kind) {
            is_write = true;
        }
    }
    if !is_write {
        return;
    }
    if is_pure {
        out.push(DiagnosticData {
            span,
            message: format!("pure function '{func_name}' cannot write to state variable '{root}'"),
        });
        return;
    }
    if is_view {
        out.push(DiagnosticData {
            span,
            message: format!("view function '{func_name}' cannot write to state variable '{root}'"),
        });
    }
}

fn next_code_after(tokens: &[forgels_syntax::Token], end: u32) -> Option<forgels_syntax::Token> {
    let comment = SolidityKind::Comment.as_u16();
    for token in tokens {
        if token.kind == comment {
            continue;
        }
        if token.span.start >= end {
            return Some(token.clone());
        }
    }
    None
}

/// True when `tokens[index]` is the root of a direct member write.
///
/// Covers `arr.push(...)`, `arr.pop(...)`, and `s.field` followed by
/// an assign family. Longer chains stay false so they keep the token
/// path. Lets member diagnostics own the root and avoids a double
/// read plus write in pure functions.
fn root_has_member_write(tokens: &[forgels_syntax::Token], index: usize) -> bool {
    let comment = SolidityKind::Comment.as_u16();
    let mut dot_pos: Option<usize> = None;
    let mut at = index + 1;
    while let Some(token) = tokens.get(at) {
        if token.kind == comment {
            at += 1;
            continue;
        }
        if token.kind == SolidityKind::Dot.as_u16() {
            dot_pos = Some(at);
        }
        break;
    }
    let Some(dot) = dot_pos else {
        return false;
    };
    let mut field_pos: Option<usize> = None;
    let mut cursor = dot + 1;
    while let Some(token) = tokens.get(cursor) {
        if token.kind == comment {
            cursor += 1;
            continue;
        }
        if token.kind == SolidityKind::Ident.as_u16() {
            field_pos = Some(cursor);
        }
        break;
    }
    let Some(field_at) = field_pos else {
        return false;
    };
    let field = tokens[field_at].text.as_str();
    if field == "push" || field == "pop" {
        let mut after = field_at + 1;
        while let Some(token) = tokens.get(after) {
            if token.kind == comment {
                after += 1;
                continue;
            }
            return token.kind == SolidityKind::LParen.as_u16()
                || token.kind == SolidityKind::LBrace.as_u16();
        }
        return false;
    }
    let mut after = field_at + 1;
    while let Some(token) = tokens.get(after) {
        if token.kind == comment {
            after += 1;
            continue;
        }
        return is_write_kind(token.kind);
    }
    false
}
