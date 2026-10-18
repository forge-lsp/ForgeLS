use forgels_syntax::Span;

use super::{SymbolKind, members::MemberInfo};

/// Origin tag for address members in completion and hover details.
pub(crate) const ADDRESS_ORIGIN: &str = "solidity";

/// True for `address` and `address payable` only.
///
/// Array types such as `address[]` are not addresses; the caller checks
/// `array_shape` first so they keep array members.
pub(crate) fn is_address_type(type_text: &str) -> bool {
    let normalized = type_text.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized == "address" || normalized == "address payable"
}

/// True for `address payable` only, from the stored type text.
pub(crate) fn is_payable_address(type_text: &str) -> bool {
    let normalized = type_text.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized == "address payable"
}

/// Members for the address type: payable has eight, plain has six.
///
/// Payable adds `transfer` and `send`; plain keeps `balance`, `code`,
/// `codehash`, `call`, `delegatecall`, and `staticcall` only.
/// Spans are synthetic (`0,0`) by design: hover uses the cursor member
/// span and goto returns `None`, so they never leak to navigation.
pub(crate) fn address_members(payable: bool) -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    let mut out = vec![
        MemberInfo {
            name: String::from("balance"),
            detail: format!("uint256 balance - from {ADDRESS_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("code"),
            detail: format!("bytes code - from {ADDRESS_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("codehash"),
            detail: format!("bytes32 codehash - from {ADDRESS_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
    ];
    if payable {
        out.push(MemberInfo {
            name: String::from("transfer"),
            detail: format!("function transfer(uint256 amount) - from {ADDRESS_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        });
        out.push(MemberInfo {
            name: String::from("send"),
            detail: format!("function send(uint256 amount) - from {ADDRESS_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        });
    }
    out.push(MemberInfo {
        name: String::from("call"),
        detail: format!("function call(bytes data) - from {ADDRESS_ORIGIN}"),
        kind: SymbolKind::Function,
        span,
    });
    out.push(MemberInfo {
        name: String::from("delegatecall"),
        detail: format!("function delegatecall(bytes data) - from {ADDRESS_ORIGIN}"),
        kind: SymbolKind::Function,
        span,
    });
    out.push(MemberInfo {
        name: String::from("staticcall"),
        detail: format!("function staticcall(bytes data) - from {ADDRESS_ORIGIN}"),
        kind: SymbolKind::Function,
        span,
    });
    out
}

/// One address member by name, or `None` for unknown names.
///
/// Non-payable has no `transfer` or `send`, so those stay silent there.
pub(crate) fn address_member(name: &str, payable: bool) -> Option<MemberInfo> {
    if name.is_empty() {
        return None;
    }
    address_members(payable)
        .into_iter()
        .find(|item| item.name == name)
}

/// Expected call arity for address methods.
///
/// `call`, `delegatecall`, and `staticcall` take 1 arg always.
/// `transfer` and `send` take 1 arg on payable only, silent otherwise.
/// `balance`, `code`, and `codehash` are fields so calls stay
/// silent, and unknown names stay silent.
pub(crate) fn address_expected_arity(name: &str, payable: bool) -> Option<usize> {
    match name {
        "transfer" | "send" if !payable => None,
        "transfer" | "send" | "call" | "delegatecall" | "staticcall" => Some(1),
        _ => None,
    }
}

/// True when the cursor sits on an address member position.
///
/// Used as a goto guard so address members never jump to coincidental
/// same-name declarations in imports. Hover keeps the member span and
/// needs no guard.
pub(crate) fn is_address_at(
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
        super::members::TypeKind::Address { .. }
    )
}
