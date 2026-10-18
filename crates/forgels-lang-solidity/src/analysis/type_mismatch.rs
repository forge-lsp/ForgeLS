use forgels_core::DiagnosticData;

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

/// Single literal initializers against elementary types only.
///
/// Multi token initializers (`1 ether`), hex addresses for `address`,
/// idents, calls, casts, and binops stay silent: the parser only stores
/// one literal token, and `address` accepts any number. Non-elementary
/// types (mappings, arrays, user types) stay silent.
pub fn type_mismatch_diagnostics(file: &SolidityFile) -> Vec<DiagnosticData> {
    let mut out = Vec::new();
    for contract in file.contracts() {
        for var in contract.state_vars() {
            check_var(
                &mut out,
                var.type_node().map(|node| node.text()),
                var.initializer(),
            );
        }
        for function in contract.functions() {
            let Some(body) = function.body() else {
                continue;
            };
            for local in body.locals() {
                check_var(
                    &mut out,
                    local.type_node().map(|node| node.text()),
                    local.initializer(),
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
