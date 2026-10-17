use forgels_syntax::Span;

use super::{SymbolKind, members::MemberInfo};

/// Origin tag for array members in completion and hover details.
pub(crate) const ARRAY_ORIGIN: &str = "solidity";

/// Outer suffix decides array shape: `[]` is dynamic, `[N]` is fixed.
///
/// `uint[2][]` is dynamic (outer `[]`), `uint[][2]` is fixed (outer
/// `[2]`). Element type and data location are ignored in V1, only the
/// trailing bracket pair matters. Returns `Some(true)` for dynamic,
/// `Some(false)` for fixed, `None` when not an array type.
pub(crate) fn array_shape(type_text: &str) -> Option<bool> {
    let text = type_text.trim();
    if !text.ends_with(']') {
        return None;
    }
    let open = text.rfind('[')?;
    let inner = text.get(open + 1..text.len() - 1)?;
    let inner = inner.trim();
    if inner.contains('[') || inner.contains(']') {
        return None;
    }
    Some(inner.is_empty())
}

/// Members for an array type: `length` always, `push` and `pop` for
/// dynamic only. Fixed arrays expose `length` alone.
///
/// Spans are synthetic (`0,0`) by design: hover uses the cursor member
/// span and goto returns `None`, so they never leak to navigation.
pub(crate) fn array_members(dynamic: bool) -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    let mut out = vec![MemberInfo {
        name: String::from("length"),
        detail: format!("uint256 length - from {ARRAY_ORIGIN}"),
        kind: SymbolKind::Field,
        span,
    }];
    if !dynamic {
        return out;
    }
    out.push(MemberInfo {
        name: String::from("push"),
        detail: format!("function push(value) - from {ARRAY_ORIGIN}"),
        kind: SymbolKind::Function,
        span,
    });
    out.push(MemberInfo {
        name: String::from("pop"),
        detail: format!("function pop() - from {ARRAY_ORIGIN}"),
        kind: SymbolKind::Function,
        span,
    });
    out
}

/// One array member by name, or `None` for unknown names.
pub(crate) fn array_member(name: &str, dynamic: bool) -> Option<MemberInfo> {
    if name.is_empty() {
        return None;
    }
    array_members(dynamic)
        .into_iter()
        .find(|item| item.name == name)
}

/// Expected call arity for array methods: `push` takes 1, `pop` takes 0.
///
/// `length` is a field so calls stay silent, fixed arrays stay silent
/// for `push` and `pop`, and unknown names stay silent.
pub(crate) fn array_expected_arity(name: &str, dynamic: bool) -> Option<usize> {
    if !dynamic {
        return None;
    }
    match name {
        "push" => Some(1),
        "pop" => Some(0),
        _ => None,
    }
}

/// True when the cursor sits on an array member position.
///
/// Used as a goto guard so array members never jump to coincidental
/// same-name declarations in imports. Hover keeps the member span and
/// needs no guard.
pub(crate) fn is_array_at(
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
        super::members::TypeKind::Array { .. }
    )
}
