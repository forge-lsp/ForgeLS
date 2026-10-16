use super::{SymbolKind, collect_symbols, is_word_byte};
use crate::{SolidityFile, SolidityLexer, SolidityParser};
use forgels_syntax::Lexer;

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
