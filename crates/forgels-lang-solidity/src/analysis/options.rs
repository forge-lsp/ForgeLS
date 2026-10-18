use forgels_core::DiagnosticData;
use forgels_lint::RuleId;
use forgels_syntax::{Lexer, Span, Token};

use crate::{SolidityFile, SolidityLexer, lexer::SolidityKind};

/// Option keys allowed on plain and member calls.
const CALL_OPTIONS: &[&str] = &["value", "gas"];

/// Option keys allowed on `new`: call keys plus `salt` for create2.
const NEW_OPTIONS: &[&str] = &["value", "gas", "salt"];

/// Option keys inside one `{...}` span: name plus key span.
///
/// Segments split on top-level commas so nested calls never break
/// keys. A key is the first ident before a colon; segments without
/// that shape stay silent.
fn option_keys(text: &str, span: Span) -> Vec<(String, Span)> {
    let comment = SolidityKind::Comment.as_u16();
    let mut inner: Vec<Token> = Vec::new();
    for token in SolidityLexer.tokenize(text) {
        if !span.contains(token.span.start) {
            continue;
        }
        if token.kind == comment {
            continue;
        }
        inner.push(token);
    }
    let Some(open) = inner
        .iter()
        .position(|token| token.kind == SolidityKind::LBrace.as_u16())
    else {
        return Vec::new();
    };
    let Some(close) = inner
        .iter()
        .rposition(|token| token.kind == SolidityKind::RBrace.as_u16())
    else {
        return Vec::new();
    };
    if open >= close {
        return Vec::new();
    }
    let mut segments: Vec<Vec<Token>> = vec![Vec::new()];
    let mut depth = 0u32;
    for token in inner.into_iter().skip(open + 1) {
        if token.span.start >= span.end {
            break;
        }
        match token.kind {
            kind if kind == SolidityKind::LParen.as_u16()
                || kind == SolidityKind::LBracket.as_u16()
                || kind == SolidityKind::LBrace.as_u16() =>
            {
                depth += 1;
                if let Some(last) = segments.last_mut() {
                    last.push(token);
                }
            }
            kind if kind == SolidityKind::RParen.as_u16()
                || kind == SolidityKind::RBracket.as_u16()
                || kind == SolidityKind::RBrace.as_u16() =>
            {
                depth = depth.saturating_sub(1);
                if let Some(last) = segments.last_mut() {
                    last.push(token);
                }
            }
            kind if kind == SolidityKind::Comma.as_u16() && depth == 0 => {
                segments.push(Vec::new());
            }
            _ => {
                if let Some(last) = segments.last_mut() {
                    last.push(token);
                }
            }
        }
    }
    let mut out = Vec::new();
    for segment in &segments {
        let Some(at) = segment
            .iter()
            .position(|token| token.kind == SolidityKind::Ident.as_u16())
        else {
            continue;
        };
        let is_key = segment
            .get(at + 1)
            .is_some_and(|next| next.kind == SolidityKind::Colon.as_u16());
        if !is_key {
            continue;
        }
        let key = &segment[at];
        out.push((key.text.clone(), key.span));
    }
    out
}

fn check_keys(text: &str, span: Span, is_new: bool, out: &mut Vec<DiagnosticData>) {
    let allowed = if is_new { NEW_OPTIONS } else { CALL_OPTIONS };
    for (key, key_span) in option_keys(text, span) {
        if key.is_empty() {
            continue;
        }
        if allowed.contains(&key.as_str()) {
            continue;
        }
        if key == "salt" {
            out.push(DiagnosticData::error(
                key_span,
                String::from("call option 'salt' only allowed for 'new'"),
                RuleId::CALL_OPTIONS,
            ));
        } else {
            out.push(DiagnosticData::error(
                key_span,
                format!("unknown call option '{key}'"),
                RuleId::CALL_OPTIONS,
            ));
        }
    }
}

/// Call option checks: unknown keys error, `salt` needs `new`.
///
/// Plain calls, member calls, and deploys each read their own
/// `{...}` span. `payable` owns the `value` check separately, so
/// this never reports it and the two never double.
pub fn options_diagnostics(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for call in body.all_calls() {
                let Some(span) = call.options_span() else {
                    continue;
                };
                check_keys(text, span, false, &mut out);
            }
            for member in body.all_members() {
                if !member.is_call() {
                    continue;
                }
                let Some(span) = member.options_span() else {
                    continue;
                };
                check_keys(text, span, false, &mut out);
            }
            for deploy in body.all_news() {
                let Some(span) = deploy.options_span() else {
                    continue;
                };
                check_keys(text, span, true, &mut out);
            }
        }
    }
    out
}
