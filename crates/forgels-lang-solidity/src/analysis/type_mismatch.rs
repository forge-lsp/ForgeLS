use forgels_core::DiagnosticData;
use forgels_syntax::{AstNode, Span};

use crate::{SolidityFile, lexer::SolidityKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Elementary {
    Bool,
    Number,
    Address,
    Str,
    Bytes,
}

fn elementary_of(type_text: &str) -> Option<Elementary> {
    let text = type_text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains('[') || text.contains(']') {
        return None;
    }
    if text.starts_with("mapping") || text.contains("=>") {
        return None;
    }
    let first = text.split_whitespace().next().unwrap_or("");
    if first == "bool" {
        return Some(Elementary::Bool);
    }
    if first == "address" {
        return Some(Elementary::Address);
    }
    if first == "string" {
        return Some(Elementary::Str);
    }
    if first == "bytes" || first.starts_with("bytes") {
        return Some(Elementary::Bytes);
    }
    if first == "uint"
        || first == "int"
        || first == "fixed"
        || first == "ufixed"
        || first.starts_with("uint")
        || first.starts_with("int")
        || first.starts_with("fixed")
        || first.starts_with("ufixed")
    {
        // Sized forms share the bare prefix, so match the full word:
        // uint8 and fixed128x18 are numeric, other spellings are not.
        if is_numeric_word(first) {
            return Some(Elementary::Number);
        }
        return None;
    }
    None
}

fn is_numeric_word(word: &str) -> bool {
    if word == "uint" || word == "int" || word == "fixed" || word == "ufixed" {
        return true;
    }
    if let Some(bits) = word.strip_prefix("uint")
        && !bits.is_empty()
        && bits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return true;
    }
    if let Some(bits) = word.strip_prefix("int")
        && !bits.is_empty()
        && bits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return true;
    }
    if word == "fixed128x18" || word == "ufixed128x18" {
        return true;
    }
    if word.starts_with("fixed") || word.starts_with("ufixed") {
        // Fixed MxN forms are numeric; anything else is a user type.
        let rest = word
            .strip_prefix("ufixed")
            .or_else(|| word.strip_prefix("fixed"));
        if let Some(rest) = rest
            && rest.contains('x')
        {
            return true;
        }
        return false;
    }
    false
}

fn literal_label(kind: u16) -> &'static str {
    if kind == SolidityKind::Number.as_u16() || kind == SolidityKind::OctalNumber.as_u16() {
        "number literal"
    } else if kind == SolidityKind::StringLit.as_u16()
        || kind == SolidityKind::UnicodeString.as_u16()
    {
        "string literal"
    } else if kind == SolidityKind::HexString.as_u16() {
        "hex literal"
    } else {
        "bool literal"
    }
}

fn matches(elementary: Elementary, kind: u16) -> bool {
    match elementary {
        Elementary::Bool => {
            kind == SolidityKind::True.as_u16() || kind == SolidityKind::False.as_u16()
        }
        Elementary::Number => {
            kind == SolidityKind::Number.as_u16() || kind == SolidityKind::OctalNumber.as_u16()
        }
        Elementary::Address => {
            kind == SolidityKind::Number.as_u16() || kind == SolidityKind::OctalNumber.as_u16()
        }
        Elementary::Str => {
            kind == SolidityKind::StringLit.as_u16() || kind == SolidityKind::UnicodeString.as_u16()
        }
        Elementary::Bytes => {
            kind == SolidityKind::Number.as_u16()
                || kind == SolidityKind::OctalNumber.as_u16()
                || kind == SolidityKind::StringLit.as_u16()
                || kind == SolidityKind::HexString.as_u16()
        }
    }
}

/// Explicit casts stay silent: `address(0)` never mismatches.
///
/// Any `Type(...)` wrapper counts as a conversion, so the check
/// below returns early and never infers a builtin type from it.
fn is_cast_expr(expr: &str) -> bool {
    let text = expr.trim();
    if text.is_empty() {
        return false;
    }
    if !text.ends_with(')') {
        return false;
    }
    let Some(open) = text.find('(') else {
        return false;
    };
    let prefix = text.get(..open).map(|part| part.trim()).unwrap_or("");
    if prefix.is_empty() {
        return false;
    }
    let first = prefix.split_whitespace().next().unwrap_or("");
    if first.is_empty() {
        return false;
    }
    elementary_of(first).is_some()
}

/// Tiny builtin type table for initializers.
///
/// Only `msg.sender` as address, `block.coinbase` as address,
/// eight `block.*` numbers as `uint256`, any `abi.encode*` call as
/// `bytes`, plus `*.balance` and `*.length` as `uint256`. Everything
/// else stays silent: literal chains, binops, plain calls, and call
/// returns are deferred by design.
fn builtin_init_type(expr: &str) -> Option<(Elementary, &'static str)> {
    let text = expr.trim();
    if text.is_empty() {
        return None;
    }
    if is_cast_expr(text) {
        return None;
    }
    if text == "msg.sender" {
        return Some((Elementary::Address, "address"));
    }
    if text.starts_with("block.") && !text.contains('(') && !text.contains(' ') {
        let rest = text.get("block.".len()..).unwrap_or("");
        match rest {
            "coinbase" => return Some((Elementary::Address, "address")),
            "basefee" | "blobbasefee" | "chainid" | "difficulty" | "gaslimit" | "number"
            | "timestamp" | "prevrandao" => return Some((Elementary::Number, "uint256")),
            _ => return None,
        }
    }
    if text.starts_with("abi.encode") && text.contains('(') {
        return Some((Elementary::Bytes, "bytes"));
    }
    if text.ends_with(".balance") && !text.contains('(') && !text.contains(' ') {
        return Some((Elementary::Number, "uint256"));
    }
    if text.ends_with(".length") && !text.contains('(') && !text.contains(' ') {
        return Some((Elementary::Number, "uint256"));
    }
    None
}

/// Raw initializer text between `=` and `;` inside `var_span`.
///
/// Finds the last assign `=` that is not part of `=>` or `==`, then
/// slices to the closing `;`. Returns the trimmed text plus its span.
/// No init or mapping-only `=>` gives `None`.
fn init_expr(text: &str, var_span: Span) -> Option<(String, Span)> {
    let start = var_span.start as usize;
    let end = (var_span.end as usize).min(text.len());
    if start >= end {
        return None;
    }
    let slice = text.get(start..end)?;
    let semi_rel = slice.rfind(';')?;
    let bytes = slice.as_bytes();
    let mut eq_rel: Option<usize> = None;
    let mut index = 0;
    while index < semi_rel {
        if bytes[index] == b'=' {
            let prev = if index > 0 { bytes[index - 1] } else { b' ' };
            let next = bytes.get(index + 1).copied().unwrap_or(b' ');
            if next != b'>' && next != b'=' && prev != b'=' && prev != b'>' {
                eq_rel = Some(index);
            }
        }
        index += 1;
    }
    let eq = eq_rel?;
    let raw = slice.get(eq + 1..semi_rel)?;
    let leading = raw.len() - raw.trim_start().len();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let expr_start = (start + eq + 1 + leading) as u32;
    let expr_end = (expr_start as usize + trimmed.len()) as u32;
    Some((trimmed.to_string(), Span::new(expr_start, expr_end)))
}

/// Single literal initializers plus tiny builtin inference.
///
/// Literals keep the old elementary check. Non-literals infer only
/// the five builtin shapes above and compare against the declared
/// elementary type. Casts, chains, binops, and calls stay silent.
pub fn type_mismatch_diagnostics(file: &SolidityFile, text: &str) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for var in contract.state_vars() {
            check_var(
                &mut out,
                var.type_node().map(|node| node.text()),
                var.initializer(),
            );
            if var.initializer().is_some() {
                continue;
            }
            check_builtin(
                &mut out,
                text,
                var.type_node().map(|node| node.text()),
                var.syntax().span,
            );
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for local in body.all_locals() {
                check_var(
                    &mut out,
                    local.type_node().map(|node| node.text()),
                    local.initializer(),
                );
                if local.initializer().is_some() {
                    continue;
                }
                check_builtin(
                    &mut out,
                    text,
                    local.type_node().map(|node| node.text()),
                    local.syntax().span,
                );
            }
        }
    }
    out
}

fn check_var(
    out: &mut Vec<DiagnosticData>,
    type_text: Option<String>,
    init: Option<forgels_syntax::SyntaxToken>,
) {
    let Some(type_text) = type_text else {
        return;
    };
    let Some(literal) = init else {
        return;
    };
    let Some(elementary) = elementary_of(&type_text) else {
        return;
    };
    if matches(elementary, literal.kind) {
        return;
    }
    out.push(DiagnosticData {
        span: literal.span,
        message: format!(
            "type mismatch: cannot assign {} to '{type_text}'",
            literal_label(literal.kind)
        ),
    });
}

fn check_builtin(
    out: &mut Vec<DiagnosticData>,
    text: &str,
    type_text: Option<String>,
    var_span: Span,
) {
    let Some(type_text) = type_text else {
        return;
    };
    let Some(declared) = elementary_of(&type_text) else {
        return;
    };
    let Some((expr, span)) = init_expr(text, var_span) else {
        return;
    };
    let Some((inferred, label)) = builtin_init_type(&expr) else {
        return;
    };
    if inferred == declared {
        return;
    }
    out.push(DiagnosticData {
        span,
        message: format!("type mismatch: cannot assign {label} to '{type_text}'"),
    });
}
