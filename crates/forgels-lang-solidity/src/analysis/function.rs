use forgels_syntax::Span;

use super::{SymbolKind, members::MemberInfo};

/// Origin tag for function type members in completion and hover details.
pub(crate) const FUNCTION_ORIGIN: &str = "solidity";

/// True when the stored type is a function type.
///
/// Matches the `function` prefix only. Visibility and returns are
/// ignored here; the external gate lives in `is_external_function_type`.
pub(crate) fn is_function_type(type_text: &str) -> bool {
    type_text.trim_start().starts_with("function")
}

/// True for external function types only.
///
/// V1 exposes `selector`, `gas`, and `value` for external types.
/// Internal types stay unknown and yield no members.
pub(crate) fn is_external_function_type(type_text: &str) -> bool {
    if !is_function_type(type_text) {
        return false;
    }
    type_text.contains("external")
}

/// Members for external function types: `selector` plus `gas` and `value`.
///
/// `selector` is a field, `gas` and `value` take 1 arg each. Spans are
/// synthetic (`0,0`) by design: hover uses the cursor member span and
/// goto returns `None`.
pub(crate) fn function_members() -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    vec![
        MemberInfo {
            name: String::from("selector"),
            detail: format!("bytes4 selector - from {FUNCTION_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("gas"),
            detail: format!("function gas(uint256 gasAmount) - from {FUNCTION_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("value"),
            detail: format!("function value(uint256 amount) - from {FUNCTION_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
    ]
}

/// One function member by name, or `None` for unknown names.
pub(crate) fn function_member(name: &str) -> Option<MemberInfo> {
    if name.is_empty() {
        return None;
    }
    function_members()
        .into_iter()
        .find(|item| item.name == name)
}

/// Expected call arity for function options.
///
/// `gas` and `value` take 1 arg each. `selector` is a field so calls
/// stay silent, and unknown names stay silent.
pub(crate) fn function_expected_arity(name: &str) -> Option<usize> {
    match name {
        "gas" | "value" => Some(1),
        _ => None,
    }
}

/// True when the cursor sits on a function type member position.
pub(crate) fn is_function_at(
    file: &crate::SolidityFile,
    text: &str,
    offset: u32,
    scope: &super::Scope,
) -> bool {
    let Some(ctx) = super::members::member_context_at(text, offset) else {
        return false;
    };
    if !ctx.member_span.contains(offset) && ctx.member_span.end != offset {
        return false;
    }
    matches!(
        super::members::receiver_type(file, scope, offset, &ctx.receiver),
        super::members::TypeKind::Function
    )
}
