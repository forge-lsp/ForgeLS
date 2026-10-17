use forgels_syntax::Span;

use super::{SymbolKind, members::MemberInfo};

/// Origin tag for `type()` members in completion and hover details.
pub(crate) const META_ORIGIN: &str = "solidity";

/// Member position inside `type(Inner).member`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeContext {
    pub inner: String,
    pub inner_span: Span,
    pub member: String,
    pub member_span: Span,
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Member context for `type(Inner).member` at `offset`.
///
/// Normal `member_context_at` fails here: the receiver ends with `)`
/// so no word precedes the dot. This scan finds the member word after
/// the dot, then walks back through `)` to the matching `(` and checks
/// for the `type` keyword. Inner types with nested parens return `None`.
/// Empty members (`type(X).`) return a context with an empty member so
/// completion still lists the inner dispatch set.
pub fn type_context_at(text: &str, offset: u32) -> Option<TypeContext> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let at = (offset as usize).min(len);
    let (member_start, member_end) = if at < len && is_word_byte(bytes[at]) {
        let mut start = at;
        while start > 0 && is_word_byte(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = at;
        while end < len && is_word_byte(bytes[end]) {
            end += 1;
        }
        (start, end)
    } else {
        let mut start = at;
        while start > 0 && start <= len && is_word_byte(bytes[start - 1]) {
            start -= 1;
        }
        (start, at)
    };
    if member_start == 0 || bytes[member_start - 1] != b'.' {
        return None;
    }
    let dot = member_start - 1;
    if dot == 0 || bytes[dot - 1] != b')' {
        return None;
    }
    let close = dot - 1;
    let mut depth: usize = 0;
    let mut open: Option<usize> = None;
    let mut index = close;
    loop {
        match bytes[index] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 {
                    open = Some(index);
                    break;
                }
            }
            _ => {}
        }
        if index == 0 {
            break;
        }
        index -= 1;
    }
    let open = open?;
    let inner = text.get(open + 1..close)?;
    if inner.contains('(') || inner.contains(')') {
        return None;
    }
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut before = open;
    while before > 0 && bytes[before - 1].is_ascii_whitespace() {
        before -= 1;
    }
    let mut word_start = before;
    while word_start > 0 && is_word_byte(bytes[word_start - 1]) {
        word_start -= 1;
    }
    if text.get(word_start..before) != Some("type") {
        return None;
    }
    let leading = inner.len() - inner.trim_start().len();
    let inner_span = Span::new(
        (open + 1 + leading) as u32,
        (open + 1 + leading + trimmed.len()) as u32,
    );
    let member = text.get(member_start..member_end).map(str::to_string)?;
    Some(TypeContext {
        inner: trimmed.to_string(),
        inner_span,
        member,
        member_span: Span::new(member_start as u32, member_end as u32),
    })
}

/// True for integer inners: `uint`, `int`, and sized forms.
///
/// Bare `uint` and `int` alias the 256-bit forms. Sized values run
/// 8 to 256 in steps of 8. Anything else dispatches to the contract set.
fn is_integer_inner(inner: &str) -> bool {
    let text = inner.trim();
    if text == "uint" || text == "int" {
        return true;
    }
    for prefix in ["uint", "int"] {
        let Some(rest) = text.strip_prefix(prefix) else {
            continue;
        };
        if rest.is_empty() || rest.contains(' ') || rest.contains('[') {
            continue;
        }
        let Ok(size): Result<u32, _> = rest.parse() else {
            continue;
        };
        if (8..=256).contains(&size) && size % 8 == 0 {
            return true;
        }
    }
    false
}

/// Members for `type(Inner)`: `min` and `max` for integers, contract
/// metadata otherwise.
///
/// Integer inners yield `min` and `max` typed as the inner itself.
/// Contract-like inners yield `name`, `creationCode`, `runtimeCode`,
/// and `interfaceId` in one set: V1 does not split contracts from
/// interfaces. Enum inners fall into the contract set as follow-up.
/// Spans are synthetic (`0,0`) by design.
pub(crate) fn meta_members_for(inner: &str) -> Vec<MemberInfo> {
    let span = Span::new(0, 0);
    if is_integer_inner(inner) {
        let trimmed = inner.trim().to_string();
        return vec![
            MemberInfo {
                name: String::from("min"),
                detail: format!("{trimmed} min - from {META_ORIGIN}"),
                kind: SymbolKind::Field,
                span,
            },
            MemberInfo {
                name: String::from("max"),
                detail: format!("{trimmed} max - from {META_ORIGIN}"),
                kind: SymbolKind::Field,
                span,
            },
        ];
    }
    vec![
        MemberInfo {
            name: String::from("name"),
            detail: format!("string name - from {META_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("creationCode"),
            detail: format!("bytes creationCode - from {META_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("runtimeCode"),
            detail: format!("bytes runtimeCode - from {META_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
        MemberInfo {
            name: String::from("interfaceId"),
            detail: format!("bytes4 interfaceId - from {META_ORIGIN}"),
            kind: SymbolKind::Field,
            span,
        },
    ]
}

/// One `type(Inner)` member by name, or `None` for unknown names.
pub(crate) fn meta_member(inner: &str, name: &str) -> Option<MemberInfo> {
    if inner.trim().is_empty() || name.is_empty() {
        return None;
    }
    meta_members_for(inner)
        .into_iter()
        .find(|item| item.name == name)
}

/// Expected call arity for `type()` members: always silent.
///
/// All meta members are fields, so calls stay silent.
pub(crate) fn meta_expected_arity(inner: &str, name: &str) -> Option<usize> {
    let _ = (inner, name);
    None
}

/// True when the cursor sits on a `type(Inner).member` position.
pub(crate) fn is_meta_at(text: &str, offset: u32) -> bool {
    let Some(ctx) = type_context_at(text, offset) else {
        return false;
    };
    ctx.member_span.contains(offset) || ctx.member_span.end == offset
}
