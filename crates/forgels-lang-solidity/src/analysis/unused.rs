use std::collections::HashSet;
use std::path::{Path, PathBuf};

use forgels_core::{DiagnosticData, ProjectContext};
use forgels_syntax::{AstNode, Lexer};

use super::{forge, references, unknown};
use crate::{SolidityFile, SolidityLexer};

fn is_builtin_word(name: &str) -> bool {
    if unknown::GLOBAL_RECEIVERS.contains(&name) || unknown::ALWAYS_OK.contains(&name) {
        return true;
    }
    forge::ASSERT_TABLE.iter().any(|item| item.name == name)
        || forge::VM_TABLE.iter().any(|item| item.name == name)
        || forge::CONSOLE_TABLE.iter().any(|item| item.name == name)
}

fn has_non_decl_use(refs: &[forgels_core::ReferenceData]) -> bool {
    refs.iter().any(|item| !item.is_declaration)
}

fn is_used(
    file: &SolidityFile,
    text: &str,
    offset: u32,
    current: &Path,
    deps: &[(PathBuf, SolidityFile)],
    ctx: &ProjectContext,
) -> bool {
    let Some(refs) = references::find_references(file, text, offset, current, deps, ctx) else {
        return true;
    };
    has_non_decl_use(&refs)
}

fn push_unused(
    out: &mut Vec<DiagnosticData>,
    seen: &mut HashSet<(u32, u32)>,
    name: &str,
    span: Option<forgels_syntax::Span>,
    is_param: bool,
) {
    let Some(span) = span else {
        return;
    };
    if name.is_empty() {
        return;
    }
    if is_builtin_word(name) {
        return;
    }
    if !seen.insert((span.start, span.end)) {
        return;
    }
    let kind = if is_param { "parameter" } else { "variable" };
    out.push(DiagnosticData {
        span,
        message: format!("unused {kind} '{name}'"),
    });
}

/// Unused locals, params, and imports.
///
/// Params and locals reuse `find_references`: a decl with no
/// non-declaration reference errors. State vars, functions, structs,
/// and return params stay silent. Imports use a token scan: a named
/// import with no usage outside its span errors once per directive.
/// Plain `import "./A.sol";` stays silent. Results dedupe by span.
pub fn unused_diagnostics_ctx(
    file: &SolidityFile,
    text: &str,
    _scope: &super::Scope,
    deps: &[(PathBuf, SolidityFile)],
    ctx: &ProjectContext,
) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for param in function.params() {
                let Some(name) = param.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let Some(span) = param.name_span().or_else(|| Some(param.syntax().span)) else {
                    continue;
                };
                if is_builtin_word(&name) {
                    continue;
                }
                if is_used(file, text, span.start, ctx.path, deps, ctx) {
                    continue;
                }
                push_unused(&mut out, &mut seen, &name, Some(span), true);
            }
            let _ = &body;
            for local in body.all_locals() {
                let Some(name) = local.name() else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let Some(span) = local.name_span().or_else(|| Some(local.syntax().span)) else {
                    continue;
                };
                if is_builtin_word(&name) {
                    continue;
                }
                if is_used(file, text, span.start, ctx.path, deps, ctx) {
                    continue;
                }
                push_unused(&mut out, &mut seen, &name, Some(span), false);
            }
        }
    }
    out.extend(unused_imports(file, text));
    out
}

fn unused_imports(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let tokens = SolidityLexer.tokenize(text);
    let import_spans: Vec<forgels_syntax::Span> = file
        .imports()
        .iter()
        .map(|item| item.syntax().span)
        .collect();
    let decls = unknown::decl_spans(file);
    let decl_set: HashSet<(u32, u32)> = decls.iter().map(|span| (span.start, span.end)).collect();
    let mut used: HashSet<String> = HashSet::new();
    for token in &tokens {
        if token.kind != crate::SolidityKind::Ident.as_u16() {
            continue;
        }
        if token.text.is_empty() {
            continue;
        }
        if unknown::inside_any(&import_spans, token.span.start) {
            continue;
        }
        if decl_set.contains(&(token.span.start, token.span.end)) {
            continue;
        }
        used.insert(token.text.clone());
    }
    let mut out = Vec::new();
    let mut seen_path: HashSet<(u32, u32)> = HashSet::new();
    for item in file.imports() {
        let symbols = item.symbols();
        if symbols.is_empty() {
            continue;
        }
        let mut live = false;
        for name in &symbols {
            if used.contains(name) {
                live = true;
                break;
            }
        }
        if live {
            continue;
        }
        let span = item.path_span().unwrap_or_else(|| item.syntax().span);
        if !seen_path.insert((span.start, span.end)) {
            continue;
        }
        let raw = item.path().unwrap_or_default();
        out.push(DiagnosticData {
            span,
            message: format!("unused import '{raw}'"),
        });
    }
    out
}
