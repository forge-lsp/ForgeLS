use super::SymbolKind;
use crate::{SolidityKind, SolidityLexer};
use forgels_syntax::Lexer;

/// Docs attach only to these declaration kinds.
pub(crate) fn allows_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Contract
            | SymbolKind::Function
            | SymbolKind::StateVar
            | SymbolKind::Struct
            | SymbolKind::Enum
            | SymbolKind::Error
            | SymbolKind::Event
    )
}

/// NatSpec docs directly above `decl_start`, if any.
///
/// Uses raw `Comment` tokens: `///` line runs plus `/** */` blocks.
/// Gap must be tight: only whitespace with at most one blank line
/// between docs and decl, same between run lines. Markers are
/// stripped, tags stay verbatim, result is plaintext.
pub(crate) fn natspec_for(text: &str, decl_start: u32) -> Option<String> {
    let tokens = SolidityLexer.tokenize(text);
    let mut docs: Vec<(u32, u32, String)> = Vec::new();
    for token in &tokens {
        if token.kind != SolidityKind::Comment.as_u16() {
            continue;
        }
        if token.span.end > decl_start {
            continue;
        }
        let Some(stripped) = strip_comment(&token.text) else {
            continue;
        };
        if stripped.is_empty() {
            continue;
        }
        docs.push((token.span.start, token.span.end, stripped));
    }
    if docs.is_empty() {
        return None;
    }
    collect_run(text, &docs, decl_start)
}

fn collect_run(text: &str, docs: &[(u32, u32, String)], decl_start: u32) -> Option<String> {
    let mut picked: Vec<String> = Vec::new();
    let mut cursor = decl_start;
    for (start, end, body) in docs.iter().rev() {
        if !gap_ok(text, *end, cursor) {
            if picked.is_empty() {
                return None;
            }
            break;
        }
        picked.push(body.clone());
        cursor = *start;
    }
    if picked.is_empty() {
        return None;
    }
    picked.reverse();
    let joined = picked.join("\n");
    if joined.trim().is_empty() {
        return None;
    }
    Some(joined)
}

/// Tight gap: whitespace only with at most one blank line.
fn gap_ok(text: &str, from: u32, to: u32) -> bool {
    if from > to {
        return false;
    }
    let slice = match text.get(from as usize..to as usize) {
        Some(part) => part,
        None => return false,
    };
    if !slice.chars().all(|ch| ch.is_whitespace()) {
        return false;
    }
    slice.chars().filter(|ch| *ch == '\n').count() <= 2
}

/// Strip `///` or `/** */` markers, keep tags verbatim.
fn strip_comment(raw: &str) -> Option<String> {
    if raw.starts_with("////") {
        return None;
    }
    if raw.starts_with("///") {
        let after = raw.get(3..).unwrap_or("");
        let body = after.strip_prefix(' ').unwrap_or(after);
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(trimmed.to_string());
    }
    if raw.starts_with("/**/") {
        return None;
    }
    if raw.starts_with("/**") {
        let inner = raw.strip_prefix("/**").unwrap_or("");
        let inner = inner.strip_suffix("*/").unwrap_or(inner);
        let mut lines: Vec<String> = Vec::new();
        for line in inner.lines() {
            let trimmed = line.trim_start();
            let no_star = trimmed.strip_prefix('*').unwrap_or(trimmed);
            let no_space = no_star.strip_prefix(' ').unwrap_or(no_star);
            let clean = no_space.trim_end();
            if !clean.is_empty() {
                lines.push(clean.to_string());
            }
        }
        if lines.is_empty() {
            let single = inner.trim();
            if single.is_empty() {
                return None;
            }
            return Some(single.to_string());
        }
        return Some(lines.join("\n"));
    }
    None
}

/// Detail plus docs for local hover.
pub(crate) fn with_natspec(detail: &str, docs: Option<String>) -> String {
    let Some(body) = docs else {
        return detail.to_string();
    };
    if body.is_empty() {
        return detail.to_string();
    }
    format!("{detail}\n\n{body}")
}

/// Detail plus docs before the dep origin suffix.
pub(crate) fn with_natspec_origin(detail: &str, docs: Option<String>, file_name: &str) -> String {
    let Some(body) = docs else {
        return format!("{detail} - from {file_name}");
    };
    if body.is_empty() {
        return format!("{detail} - from {file_name}");
    }
    format!("{detail}\n\n{body} - from {file_name}")
}

/// Insert docs before an existing ` - from ` origin, if present.
pub(crate) fn insert_before_origin(detail: &str, docs: Option<String>) -> String {
    let Some(body) = docs else {
        return detail.to_string();
    };
    if body.is_empty() {
        return detail.to_string();
    }
    let Some(at) = detail.find(" - from ") else {
        return format!("{detail}\n\n{body}");
    };
    let (head, tail) = detail.split_at(at);
    format!("{head}\n\n{body}{tail}")
}
