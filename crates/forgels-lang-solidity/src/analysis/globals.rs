use forgels_syntax::Span;

use super::{SymbolKind, members::MemberInfo};

/// Origin tag for global members in completion and hover details.
pub(crate) const GLOBAL_ORIGIN: &str = "solidity";

/// True for the four global receivers: `msg`, `block`, `tx`, `abi`.
pub(crate) fn is_global_receiver(name: &str) -> bool {
    matches!(name, "msg" | "block" | "tx" | "abi")
}

/// Members for `msg`: sender, value, data, sig.
fn msg_members() -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    vec![
        MemberInfo {
            name: String::from("sender"),
            detail: format!("address sender - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("value"),
            detail: format!("uint256 value - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("data"),
            detail: format!("bytes data - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("sig"),
            detail: format!("bytes4 sig - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
    ]
}

/// Members for `block`: chain fields plus `blobhash`.
///
/// `blobhash` takes the blob index and returns the versioned hash.
/// All other entries are fields. `blockhash` stays a plain global
/// function, not a block member.
fn block_members() -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    vec![
        MemberInfo {
            name: String::from("basefee"),
            detail: format!("uint256 basefee - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("blobbasefee"),
            detail: format!("uint256 blobbasefee - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("chainid"),
            detail: format!("uint256 chainid - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("coinbase"),
            detail: format!("address coinbase - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("difficulty"),
            detail: format!("uint256 difficulty - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("prevrandao"),
            detail: format!("uint256 prevrandao - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("gaslimit"),
            detail: format!("uint256 gaslimit - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("number"),
            detail: format!("uint256 number - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("timestamp"),
            detail: format!("uint256 timestamp - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("blobhash"),
            detail: format!("function blobhash(uint256 index) - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
    ]
}

/// Members for `tx`: gasprice and origin.
fn tx_members() -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    vec![
        MemberInfo {
            name: String::from("gasprice"),
            detail: format!("uint256 gasprice - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("origin"),
            detail: format!("address origin - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
    ]
}

/// Members for `abi`: decode, encode family, and encodeCall.
///
/// `encode`, `encodePacked`, `encodeWithSelector`, and
/// `encodeWithSignature` take a variadic tail, so arity stays silent.
/// `decode` and `encodeCall` take 2 args each.
fn abi_members() -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    vec![
        MemberInfo {
            name: String::from("decode"),
            detail: format!("function decode(bytes data, types) - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("encode"),
            detail: format!("function encode(...) - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("encodePacked"),
            detail: format!("function encodePacked(...) - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("encodeWithSelector"),
            detail: format!(
                "function encodeWithSelector(bytes4 selector, ...) - from {GLOBAL_ORIGIN}"
            ),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("encodeWithSignature"),
            detail: format!(
                "function encodeWithSignature(string signature, ...) - from {GLOBAL_ORIGIN}"
            ),
            kind: SymbolKind::Function,
            span,
        },
        MemberInfo {
            name: String::from("encodeCall"),
            detail: format!("function encodeCall(function target, args) - from {GLOBAL_ORIGIN}"),
            kind: SymbolKind::Function,
            span,
        },
    ]
}

/// Members for a global receiver, or empty for unknown receivers.
pub(crate) fn global_members(receiver: &str) -> Vec<MemberInfo> {
    match receiver {
        "msg" => msg_members(),
        "block" => block_members(),
        "tx" => tx_members(),
        "abi" => abi_members(),
        _ => Vec::new(),
    }
}

/// One global member by name, or `None` for unknown names.
pub(crate) fn global_member(receiver: &str, name: &str) -> Option<MemberInfo> {
    if receiver.is_empty() || name.is_empty() {
        return None;
    }
    global_members(receiver)
        .into_iter()
        .find(|item| item.name == name)
}

/// Expected call arity for global methods.
///
/// `block.blobhash` takes 1, `abi.decode` and `abi.encodeCall` take 2.
/// Fields stay silent and variadic `abi.encode*` stays silent by design.
pub(crate) fn global_expected_arity(receiver: &str, name: &str) -> Option<usize> {
    match (receiver, name) {
        ("block", "blobhash") => Some(1),
        ("abi", "decode") | ("abi", "encodeCall") => Some(2),
        _ => None,
    }
}

/// True when the cursor sits on a global member position.
pub(crate) fn is_global_at(
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
        super::members::TypeKind::Global(_)
    )
}
