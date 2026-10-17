use forgels_syntax::Span;

use super::{SymbolKind, members::MemberInfo};

/// Origin tag for bytes and string members in completion and hover details.
pub(crate) const BYTES_ORIGIN: &str = "solidity";

/// True for `bytes` and `bytes1` to `bytes32` only.
///
/// Array suffixes are rejected here: the caller checks `array_shape`
/// first so `bytes[]` keeps array members. Data location is already
/// stripped from the stored type, so only the bare word matters.
pub(crate) fn is_bytes_type(type_text: &str) -> bool {
    let normalized = type_text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized == "bytes" {
        return true;
    }
    let Some(rest) = normalized.strip_prefix("bytes") else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    if rest.contains('[') || rest.contains(']') || rest.contains(' ') {
        return false;
    }
    let Ok(size): Result<u32, _> = rest.parse() else {
        return false;
    };
    (1..=32).contains(&size)
}

/// True for `string` only.
///
/// `string` without brackets is not an array: `array_shape` returns
/// `None` for it, so this check runs after the array check.
pub(crate) fn is_string_type(type_text: &str) -> bool {
    let normalized = type_text.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized == "string"
}

/// Members for bytes: `length` only.
///
/// Fixed `bytes32` shares the same set in V1: no push or pop, matching
/// the dynamic `bytes` shape. Spans are synthetic (`0,0`) by design.
pub(crate) fn bytes_members() -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    vec![MemberInfo {
        name: String::from("length"),
        detail: format!("uint256 length - from {BYTES_ORIGIN}"),
        kind: SymbolKind::Field,
        span,
    }]
}

/// Members for string: empty by design.
///
/// Solidity exposes no direct members on `string`: length needs
/// `bytes(s).length`. Completion, hover, and goto stay silent.
pub(crate) fn string_members() -> Vec<MemberInfo> {
    Vec::new()
}

/// One bytes member by name, or `None` for unknown names.
pub(crate) fn bytes_member(name: &str) -> Option<MemberInfo> {
    if name.is_empty() {
        return None;
    }
    bytes_members().into_iter().find(|item| item.name == name)
}

/// Expected call arity for bytes and string: always silent.
///
/// `length` is a field so calls stay silent, and string has no members.
pub(crate) fn bytes_expected_arity(name: &str) -> Option<usize> {
    let _ = name;
    None
}

/// True when the cursor sits on a bytes member position.
pub(crate) fn is_bytes_at(
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
        super::members::TypeKind::Bytes
    )
}

/// True when the cursor sits on a string member position.
pub(crate) fn is_string_at(
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
        super::members::TypeKind::String
    )
}
