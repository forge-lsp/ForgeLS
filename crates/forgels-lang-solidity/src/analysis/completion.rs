use std::path::{Path, PathBuf};

use super::{SymbolKind, collect_symbols, is_word_byte};
use crate::{SolidityFile, SolidityLexer, SolidityParser};
use forgels_syntax::{Lexer, Span};

pub struct CompletionItem {
    pub label: String,
    pub detail: String,
    pub kind: CompletionKind,
}

/// Origin of a completion entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Symbol(SymbolKind),
    Keyword,
    Path,
}

/// Suggest symbols visible at `offset`, filtered by the typed prefix.
///
/// Shadowed names appear once with the innermost detail. Keywords whose
/// spelling matches the prefix follow the symbols.
pub fn complete_at(file: &SolidityFile, text: &str, offset: u32) -> Vec<CompletionItem> {
    let prefix = prefix_at(text, offset);
    let scope = collect_symbols(file);
    let mut seen: Vec<String> = Vec::new();
    let mut items: Vec<CompletionItem> = Vec::new();
    for symbol in scope.visible_at(offset) {
        if symbol.name.is_empty() {
            continue;
        }
        if symbol.name.starts_with(prefix.as_str()) && !seen.contains(&symbol.name) {
            seen.push(symbol.name.clone());
            items.push(CompletionItem {
                label: symbol.name.clone(),
                detail: symbol.detail.clone(),
                kind: CompletionKind::Symbol(symbol.kind),
            });
        }
    }
    let mut keywords: Vec<&str> = crate::lexer::KEYWORDS
        .iter()
        .copied()
        .filter(|word| word.starts_with(prefix.as_str()))
        .collect();
    keywords.sort_unstable();
    for word in keywords {
        items.push(CompletionItem {
            label: word.to_string(),
            detail: "keyword".to_string(),
            kind: CompletionKind::Keyword,
        });
    }
    items
}

/// Typed fragment ending at `offset`: word chars directly before it.
pub(crate) fn prefix_at(text: &str, offset: u32) -> String {
    let end = (offset as usize).min(text.len());
    let bytes = text.as_bytes();
    let mut start = end;
    while start > 0 && is_word_byte(bytes[start - 1]) {
        start -= 1;
    }
    text.get(start..end).unwrap_or("").to_string()
}

fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.' | b'/' | b'-')
}

/// Typed path fragment ending at `offset`: path chars directly before it.
pub(crate) fn path_prefix_at(text: &str, offset: u32) -> String {
    let end = (offset as usize).min(text.len());
    let bytes = text.as_bytes();
    let mut start = end;
    while start > 0 && end <= bytes.len() && is_path_byte(bytes[start - 1]) {
        start -= 1;
    }
    text.get(start..end).unwrap_or("").to_string()
}

/// Span of the import string at `offset`, quotes included.
///
/// Parsed imports win first. The line scan fallback covers incomplete
/// `import "` rows that the parser drops.
pub fn import_path_at(file: &SolidityFile, text: &str, offset: u32) -> Option<Span> {
    for directive in file.imports() {
        let Some(span) = directive.path_span() else {
            continue;
        };
        if span.contains(offset) {
            return Some(span);
        }
    }
    scan_import_string_at(text, offset)
}

fn scan_import_string_at(text: &str, offset: u32) -> Option<Span> {
    let at = offset as usize;
    let head = text.get(..at.min(text.len()))?;
    if at > text.len() {
        return None;
    }
    let line_start = head.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let tail = text.get(at..)?;
    let line_end = tail
        .find('\n')
        .map(|index| at + index)
        .unwrap_or(text.len());
    if at < line_start || at > line_end {
        return None;
    }
    let line = text.get(line_start..line_end)?;
    let trimmed = line.trim_start();
    if !trimmed.starts_with("import") {
        return None;
    }
    let after_keyword = trimmed.get(6..).unwrap_or("");
    if let Some(next) = after_keyword.chars().next()
        && (next.is_ascii_alphanumeric() || next == '_' || next == '$')
    {
        return None;
    }
    let leading = line.len() - trimmed.len();
    let import_end = line_start + leading + 6;
    if at <= import_end {
        return None;
    }
    for quote in ['"', '\''] {
        let search_from = import_end - line_start;
        let tail = line.get(search_from..)?;
        let Some(open_rel) = tail.find(quote) else {
            continue;
        };
        let open_abs = import_end + open_rel;
        if at <= open_abs {
            continue;
        }
        let after_open = open_abs - line_start + 1;
        let rest = line.get(after_open..)?;
        if let Some(close_rel) = rest.find(quote) {
            let close_abs = open_abs + 1 + close_rel;
            if at <= close_abs {
                return Some(Span::new(open_abs as u32, (close_abs + 1) as u32));
            }
        } else if at <= line_end {
            return Some(Span::new(open_abs as u32, line_end as u32));
        }
    }
    None
}

/// Path completions for an import string.
///
/// Lists `.sol` files under `roots` (or next to `current_file` when roots
/// are empty), labels them relative to the importer, and filters by
/// `prefix`. V1 only knows `sol`.
pub fn complete_import_paths(
    current_file: &Path,
    roots: &[PathBuf],
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut search: Vec<PathBuf> = Vec::new();
    if roots.is_empty() {
        if let Some(parent) = current_file.parent() {
            search.push(parent.to_path_buf());
        }
    } else {
        for root in roots {
            if !search.contains(root) {
                search.push(root.clone());
            }
        }
        if let Some(parent) = current_file.parent()
            && !search.contains(&parent.to_path_buf())
        {
            search.push(parent.to_path_buf());
        }
    }
    if search.is_empty() {
        return Vec::new();
    }
    let caps = forgels_common::fs::WalkCaps::default();
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in &search {
        for found in forgels_common::fs::list_files_with_extension(dir, "sol", caps) {
            if !candidates.contains(&found) {
                candidates.push(found);
            }
            if candidates.len() >= caps.max_matches {
                break;
            }
        }
        if candidates.len() >= caps.max_matches {
            break;
        }
    }
    let mut items: Vec<CompletionItem> = Vec::new();
    for candidate in candidates {
        if candidate == current_file {
            continue;
        }
        let Some(label) = forgels_common::paths::relative_path(current_file, &candidate) else {
            continue;
        };
        if !label.starts_with(prefix) {
            continue;
        }
        items.push(CompletionItem {
            label,
            detail: "file".to_string(),
            kind: CompletionKind::Path,
        });
    }
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items.truncate(caps.max_matches);
    items
}

/// Contract level exports of `text`: contracts and their functions.
///
/// Parses with recovery so broken deps still contribute. Locals, params,
/// and state vars stay local and never cross files. Caller filters by
/// prefix and tags the origin file.
pub fn exports_of(text: &str) -> Vec<CompletionItem> {
    let tokens = SolidityLexer.tokenize(text);
    let (file, _) = SolidityParser.parse_with_errors(&tokens);
    let scope = collect_symbols(&file);
    scope
        .export_symbols()
        .iter()
        .map(|symbol| CompletionItem {
            label: symbol.name.clone(),
            detail: symbol.detail.clone(),
            kind: CompletionKind::Symbol(symbol.kind),
        })
        .collect()
}
