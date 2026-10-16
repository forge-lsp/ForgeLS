use std::path::{Path, PathBuf};

use super::{SymbolKind, collect_symbols, is_word_byte};
use crate::{SolidityFile, SolidityLexer, SolidityParser};
use forgels_protocol::CompletionItemKind;
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

/// Map a completion origin to its LSP number.
///
/// State vars stay Variable (6), same as locals and params.
/// Enum members map to 20 once the symbol exists (follow-up).
pub fn completion_item_kind(kind: CompletionKind) -> CompletionItemKind {
    match kind {
        CompletionKind::Keyword => CompletionItemKind::KEYWORD,
        CompletionKind::Path => CompletionItemKind::FILE,
        CompletionKind::Symbol(symbol) => match symbol {
            SymbolKind::Contract => CompletionItemKind::CLASS,
            SymbolKind::Function => CompletionItemKind::FUNCTION,
            SymbolKind::StateVar | SymbolKind::LocalVar | SymbolKind::Param => {
                CompletionItemKind::VARIABLE
            }
            SymbolKind::Field => CompletionItemKind::FIELD,
            SymbolKind::Struct => CompletionItemKind::STRUCT,
            SymbolKind::Enum => CompletionItemKind::ENUM,
        },
    }
}

/// Suggest symbols visible at `offset`, filtered by the typed prefix.
///
/// Shadowed names appear once with the innermost detail. Keywords whose
/// spelling matches the prefix follow the symbols. Positions after a dot
/// suggest members of the receiver type instead of scope symbols.
pub fn complete_at(file: &SolidityFile, text: &str, offset: u32) -> Vec<CompletionItem> {
    if let Some(ctx) = super::members::member_context_at(text, offset) {
        return complete_members(file, offset, &ctx);
    }
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

fn complete_members(
    file: &SolidityFile,
    offset: u32,
    ctx: &super::members::MemberContext,
) -> Vec<CompletionItem> {
    let scope = collect_symbols(file);
    let kind = super::members::receiver_type(file, &scope, offset, &ctx.receiver);
    let mut items: Vec<CompletionItem> = super::members::members_of(file, &kind)
        .into_iter()
        .filter(|member| member.name.starts_with(ctx.member.as_str()))
        .map(|member| CompletionItem {
            label: member.name,
            detail: member.detail,
            kind: CompletionKind::Symbol(member.kind),
        })
        .collect();
    items.sort_by(|left, right| left.label.cmp(&right.label));
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
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.' | b'/' | b'-' | b'@')
}

/// Typed path fragment ending at `offset`: path chars directly before it.
///
/// `@` counts so scoped bare imports (`@scope/pkg`) keep one prefix.
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

/// Cap for bare `node_modules` suggestions per request.
const MAX_BARE_ITEMS: usize = 32;

/// Single `read_dir` budget for one `node_modules` or scope level.
const NODE_ENTRIES_CAP: usize = 128;

/// Bare `node_modules` completions for an import string.
///
/// Relative prefixes (starting with `.`) stay empty: the relative
/// listing owns them. Otherwise the nearest readable `node_modules`
/// provides package scaffolds (`name/` or `@scope/name/`, detail
/// `package`) or `.sol` files under a package subpath (full bare
/// specifier, detail `file`). Scaffold labels are hints and may not
/// resolve; every file label must pass `resolve_import_full`.
pub fn complete_bare_imports(
    importer: &Path,
    roots: &[PathBuf],
    prefix: &str,
) -> Vec<CompletionItem> {
    if prefix.starts_with('.') {
        return Vec::new();
    }
    let Some(modules) = nearest_node_modules(importer) else {
        return Vec::new();
    };
    let root = roots
        .first()
        .map(PathBuf::as_path)
        .unwrap_or_else(|| importer.parent().unwrap_or(Path::new("")));
    let mut items = match split_bare_prefix(prefix) {
        BarePrefix::Packages => bare_package_scaffolds(&modules, prefix),
        BarePrefix::Files { head, sub } => {
            bare_package_files(importer, &modules, root, head, sub, prefix)
        }
    };
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items.truncate(MAX_BARE_ITEMS);
    items
}

/// Nearest `node_modules` the disk can read, walking up from `importer`.
///
/// Skips symlinked dirs, same stance as `is_trackable_file`.
fn nearest_node_modules(importer: &Path) -> Option<PathBuf> {
    for dir in forgels_common::paths::node_modules_dirs(importer) {
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            continue;
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            continue;
        }
        return Some(dir);
    }
    None
}

/// Top level package scaffolds: `name/` plus one `@scope/` level.
///
/// Dirs only, filtered by `prefix`, detail `package`. Trailing slash
/// marks a hint, not a resolvable import. Both levels filter by the
/// typed fragment before the cap so readdir order never hides a match.
fn bare_package_scaffolds(modules: &Path, prefix: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let scoped = prefix.starts_with('@');
    let top_filter = prefix.split('/').next().unwrap_or(prefix);
    let scope_part = if scoped && prefix.contains('/') {
        prefix.split('/').next().unwrap_or("")
    } else {
        ""
    };
    for (name, is_dir) in
        forgels_common::fs::list_dir_entries_matching(modules, top_filter, NODE_ENTRIES_CAP)
    {
        if !is_dir {
            continue;
        }
        if name.starts_with('@') {
            if !scoped && !prefix.is_empty() {
                continue;
            }
            if !scope_part.is_empty() && name != scope_part {
                continue;
            }
            let sub_filter = if scoped && prefix.contains('/') {
                let rest = prefix.split_once('/').map(|pair| pair.1).unwrap_or("");
                rest.split_once('/').map(|pair| pair.0).unwrap_or(rest)
            } else {
                ""
            };
            let scope = modules.join(&name);
            for (pkg, pkg_is_dir) in
                forgels_common::fs::list_dir_entries_matching(&scope, sub_filter, NODE_ENTRIES_CAP)
            {
                if !pkg_is_dir {
                    continue;
                }
                let label = format!("{name}/{pkg}/");
                if label.starts_with(prefix) {
                    items.push(CompletionItem {
                        label,
                        detail: "package".to_string(),
                        kind: CompletionKind::Path,
                    });
                }
            }
            continue;
        }
        let label = format!("{name}/");
        if label.starts_with(prefix) {
            items.push(CompletionItem {
                label,
                detail: "package".to_string(),
                kind: CompletionKind::Path,
            });
        }
    }
    items
}

/// `.sol` files under one package as full bare specifiers.
///
/// Walks `head` below `modules`, narrowed to the typed subdir, and
/// keeps only labels that start with `prefix` and resolve through
/// `resolve_import_full`. Detail is `file`.
fn bare_package_files(
    importer: &Path,
    modules: &Path,
    root: &Path,
    head: &str,
    sub: &str,
    prefix: &str,
) -> Vec<CompletionItem> {
    if head.is_empty() {
        return Vec::new();
    }
    if sub.split('/').any(|part| part == "..") {
        return Vec::new();
    }
    let base = modules.join(head);
    let below = sub.rfind('/').map(|at| &sub[..at]).unwrap_or("");
    let dir = if below.is_empty() {
        base.clone()
    } else {
        base.join(below)
    };
    // Match cap covers the visited budget so the `prefix` filter below
    // runs before any truncation. Caller caps the sorted result at 32.
    let visit = forgels_common::fs::WalkCaps::default().max_visited;
    let caps = forgels_common::fs::WalkCaps::new(visit, visit);
    let mut items = Vec::new();
    for found in forgels_common::fs::list_files_with_extension(&dir, "sol", caps) {
        let Ok(rel) = found.strip_prefix(&base) else {
            continue;
        };
        let parts: Vec<String> = rel
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        if parts.is_empty() {
            continue;
        }
        let label = format!("{head}/{}", parts.join("/"));
        if !label.starts_with(prefix) {
            continue;
        }
        let resolves = forgels_common::paths::resolve_import_full(
            importer,
            &label,
            root,
            &[],
            forgels_common::fs::is_trackable_file,
        );
        if resolves.is_none() {
            continue;
        }
        items.push(CompletionItem {
            label,
            detail: "file".to_string(),
            kind: CompletionKind::Path,
        });
    }
    items
}

/// Bare prefix shape: top level packages or files under one package.
enum BarePrefix<'a> {
    Packages,
    Files { head: &'a str, sub: &'a str },
}

/// Split `prefix` into a package head plus the subpath below it.
///
/// `@scope` without a second slash stays package level; `pkg/` and
/// `@scope/pkg/` count as empty subpaths so file listing takes over.
fn split_bare_prefix(prefix: &str) -> BarePrefix<'_> {
    if prefix.starts_with('@') {
        let mut parts = prefix.splitn(3, '/');
        let _scope = parts.next();
        let has_pkg = parts.next().is_some();
        if has_pkg && let Some(sub) = parts.next() {
            let head = &prefix[..prefix.len() - sub.len() - 1];
            return BarePrefix::Files { head, sub };
        }
        return BarePrefix::Packages;
    }
    if let Some(slash) = prefix.find('/') {
        return BarePrefix::Files {
            head: &prefix[..slash],
            sub: &prefix[slash + 1..],
        };
    }
    BarePrefix::Packages
}

/// Contract level exports of `text`: contracts, functions, structs, and enums.
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
