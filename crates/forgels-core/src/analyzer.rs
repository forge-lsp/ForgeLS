use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use forgels_protocol::CompletionItemKind;
use forgels_protocol::InlayHintKind;
use forgels_protocol::SymbolKind;
use forgels_syntax::Span;

use forgels_common::fs::is_trackable_file;
use forgels_common::paths::resolve_import;
use forgels_lint::Severity;

use crate::index::ProjectIndex;
use crate::routing::Language;
use crate::workspace::Workspace;

/// Hover answer from a language analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoverData {
    pub contents: String,
    pub span: Span,
}

/// Completion entry from a language analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionData {
    pub label: String,
    pub detail: String,
    pub kind: CompletionItemKind,
    pub text_edit: Option<TextEdit>,
}

/// Replacement for a completion item.
///
/// `span` covers the source range to replace, `new_text` is the full
/// replacement. Core forwards this without logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub span: Span,
    pub new_text: String,
}

impl TextEdit {
    pub fn new(span: Span, new_text: String) -> Self {
        Self { span, new_text }
    }
}

/// One parameter label inside a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParam {
    pub label: String,
}

impl SignatureParam {
    pub fn new(label: String) -> Self {
        Self { label }
    }
}

/// One callable signature: display label plus ordered params.
///
/// `active_param` repeats the cursor arg index on every entry of one
/// help response. Overloads share the cursor, so dispatch reads the
/// first entry and answers `activeSignature` 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureData {
    pub label: String,
    pub params: Vec<SignatureParam>,
    pub active_param: u32,
}

impl SignatureData {
    pub fn new(label: String, params: Vec<SignatureParam>, active_param: u32) -> Self {
        Self {
            label,
            params,
            active_param,
        }
    }
}

/// One outline entry for `textDocument/documentSymbol`.
///
/// `range` covers the whole declaration, `selection` covers the name.
/// Languages fall back to `range` when the name span is missing so
/// broken files still outline. Children nest one level per scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbolData {
    pub name: String,
    pub detail: String,
    pub kind: SymbolKind,
    pub range: Span,
    pub selection: Span,
    pub children: Vec<DocumentSymbolData>,
}

impl DocumentSymbolData {
    pub fn new(
        name: String,
        detail: String,
        kind: SymbolKind,
        range: Span,
        selection: Span,
        children: Vec<DocumentSymbolData>,
    ) -> Self {
        Self {
            name,
            detail,
            kind,
            range,
            selection,
            children,
        }
    }
}

/// Cap for `workspace/symbol` results. Bounds fan-out across files.
pub const MAX_WORKSPACE_SYMBOLS: usize = 50;

/// Cap for `textDocument/inlayHint` results. Bounds per range scan.
pub const MAX_INLAY_HINTS: usize = 100;

/// Cap for `textDocument/semanticTokens/full` results. Bounds full file scan.
pub const MAX_SEMANTIC_TOKENS: usize = 5000;

/// Cap for call hierarchy items and edges. Bounds fan-out across files.
pub const MAX_CALL_HIERARCHY: usize = 50;

/// Cap for `textDocument/codeAction` results. Bounds fix list per range.
pub const MAX_CODE_ACTIONS: usize = 10;

/// Cap for `textDocument/foldingRange` results. Bounds full file scan.
pub const MAX_FOLDING_RANGES: usize = 200;

/// Cap for `textDocument/selectionRange` chain depth. Bounds nesting.
pub const MAX_SELECTION_DEPTH: usize = 20;

/// Cap for type hierarchy items. Bounds fan-out across files.
pub const MAX_TYPE_HIERARCHY: usize = 50;

/// One quickfix: title plus a single file edit.
///
/// `kind` is always `quickfix` in V1. `edit` covers the range to
/// replace. Dispatch maps it to `WorkspaceEdit` without `command` and
/// without `resolve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionData {
    pub title: String,
    pub kind: String,
    pub edit: TextEdit,
}

impl CodeActionData {
    pub fn new(title: String, kind: String, edit: TextEdit) -> Self {
        Self { title, kind, edit }
    }
}

/// One folding range: the span to fold.
///
/// Dispatch maps `span` to `startLine` and `endLine`. Single line
/// spans never surface; languages filter them before return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRangeData {
    pub span: Span,
}

impl FoldingRangeData {
    pub fn new(span: Span) -> Self {
        Self { span }
    }
}

/// One type hierarchy item: the contract at the cursor or in the index.
///
/// `span` is the full declaration range, `selection` is the name span.
/// Dispatch maps both to LSP ranges from the owning file text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchyItemData {
    pub name: String,
    pub kind: SymbolKind,
    pub detail: String,
    pub path: PathBuf,
    pub span: Span,
    pub selection: Span,
}

impl TypeHierarchyItemData {
    pub fn new(
        name: String,
        kind: SymbolKind,
        detail: String,
        path: PathBuf,
        span: Span,
        selection: Span,
    ) -> Self {
        Self {
            name,
            kind,
            detail,
            path,
            span,
            selection,
        }
    }
}

/// One inlay hint: parameter name at an argument slot.
///
/// `offset` is the byte offset where the label renders. Dispatch maps
/// it to a position. `label` carries the trailing colon, for example
/// `to:`. Kind is always Parameter in V1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHintData {
    pub offset: u32,
    pub label: String,
    pub kind: InlayHintKind,
}

impl InlayHintData {
    pub fn new(offset: u32, label: String, kind: InlayHintKind) -> Self {
        Self {
            offset,
            label,
            kind,
        }
    }
}

/// One semantic token: classified span for highlighting.
///
/// `token_type` is an index into `SEMANTIC_TOKEN_TYPES`, `modifiers`
/// is a bitset over `SEMANTIC_TOKEN_MODIFIERS`. Dispatch delta-encodes
/// the sorted list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticTokenData {
    pub span: Span,
    pub token_type: u32,
    pub modifiers: u32,
}

impl SemanticTokenData {
    pub fn new(span: Span, token_type: u32, modifiers: u32) -> Self {
        Self {
            span,
            token_type,
            modifiers,
        }
    }
}

/// One call hierarchy item: the callable at the cursor or in the index.
///
/// `span` is the full declaration range, `selection` is the name span.
/// Dispatch maps both to LSP ranges from the owning file text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyItemData {
    pub name: String,
    pub kind: SymbolKind,
    pub detail: String,
    pub path: PathBuf,
    pub span: Span,
    pub selection: Span,
}

impl CallHierarchyItemData {
    pub fn new(
        name: String,
        kind: SymbolKind,
        detail: String,
        path: PathBuf,
        span: Span,
        selection: Span,
    ) -> Self {
        Self {
            name,
            kind,
            detail,
            path,
            span,
            selection,
        }
    }
}

/// One call hierarchy edge: caller or callee plus call sites.
///
/// `item` is the other end of the call, `ranges` holds the usage spans
/// inside the relevant file. Dispatch maps each span to `fromRanges`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHierarchyCallData {
    pub item: CallHierarchyItemData,
    pub ranges: Vec<Span>,
}

impl CallHierarchyCallData {
    pub fn new(item: CallHierarchyItemData, ranges: Vec<Span>) -> Self {
        Self { item, ranges }
    }
}

/// One flat entry for `workspace/symbol`.
///
/// `span` is the name span when valid, else the full range. `container`
/// holds the parent name for depth 1 entries, none for top level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSymbolData {
    pub name: String,
    pub kind: SymbolKind,
    pub container: Option<String>,
    pub path: PathBuf,
    pub span: Span,
}

impl WorkspaceSymbolData {
    pub fn new(
        name: String,
        kind: SymbolKind,
        container: Option<String>,
        path: PathBuf,
        span: Span,
    ) -> Self {
        Self {
            name,
            kind,
            container,
            path,
            span,
        }
    }
}

/// Flat workspace search over tracked files.
///
/// Reuses `Analyzer::document_symbols` per file, flattens depth 0-1,
/// matches substring case-insensitive on name. Files run path-sorted
/// for deterministic output. Empty query returns empty. Caps at
/// `MAX_WORKSPACE_SYMBOLS` with early exit. Skips missing analyzers
/// and out-of-bounds spans.
pub fn workspace_symbols(
    workspace: &Workspace,
    analyzers: &AnalyzerMap,
    query: &str,
) -> Vec<WorkspaceSymbolData> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut files: Vec<_> = workspace.iter_files().collect();
    files.sort_by(|left, right| left.0.cmp(right.0));
    let mut out: Vec<WorkspaceSymbolData> = Vec::new();
    for (path, doc) in files {
        if out.len() >= MAX_WORKSPACE_SYMBOLS {
            break;
        }
        let Some(analyzer) = analyzers.get(&doc.language) else {
            continue;
        };
        for top in analyzer.document_symbols(&doc.text) {
            if out.len() >= MAX_WORKSPACE_SYMBOLS {
                break;
            }
            push_if_match(&mut out, &needle, path, None, &top, &doc.text);
            for child in &top.children {
                if out.len() >= MAX_WORKSPACE_SYMBOLS {
                    break;
                }
                push_if_match(
                    &mut out,
                    &needle,
                    path,
                    Some(top.name.clone()),
                    child,
                    &doc.text,
                );
            }
        }
    }
    out
}

fn push_if_match(
    out: &mut Vec<WorkspaceSymbolData>,
    needle: &str,
    path: &Path,
    container: Option<String>,
    item: &DocumentSymbolData,
    text: &str,
) {
    if out.len() >= MAX_WORKSPACE_SYMBOLS {
        return;
    }
    if !item.name.to_lowercase().contains(needle) {
        return;
    }
    let Some(span) = pick_symbol_span(text, item) else {
        return;
    };
    out.push(WorkspaceSymbolData::new(
        item.name.clone(),
        item.kind,
        container,
        path.to_path_buf(),
        span,
    ));
}

fn pick_symbol_span(text: &str, item: &DocumentSymbolData) -> Option<Span> {
    if text
        .get(item.selection.start as usize..item.selection.end as usize)
        .is_some()
    {
        return Some(item.selection);
    }
    if text
        .get(item.range.start as usize..item.range.end as usize)
        .is_some()
    {
        return Some(item.range);
    }
    None
}

/// One parse problem from a language analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticData {
    pub span: Span,
    pub message: String,
    pub severity: Severity,
}

impl DiagnosticData {
    pub fn new(span: Span, message: String) -> Self {
        Self {
            span,
            message,
            severity: Severity::Error,
        }
    }

    pub fn error(span: Span, message: String) -> Self {
        Self {
            span,
            message,
            severity: Severity::Error,
        }
    }

    pub fn warning(span: Span, message: String) -> Self {
        Self {
            span,
            message,
            severity: Severity::Warning,
        }
    }
}

/// One import reference found in a file. `raw` keeps the literal
/// specifier from source; core resolves it against the importing file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRef {
    pub raw: String,
    pub symbols: Vec<String>,
    pub span: Span,
}

impl ImportRef {
    pub fn new(raw: String) -> Self {
        Self {
            raw,
            symbols: Vec::new(),
            span: Span::new(0, 0),
        }
    }

    pub fn with_symbols(raw: String, symbols: Vec<String>) -> Self {
        Self {
            raw,
            symbols,
            span: Span::new(0, 0),
        }
    }

    pub fn with_span(raw: String, symbols: Vec<String>, span: Span) -> Self {
        Self { raw, symbols, span }
    }
}
/// Jump target with owning file. Core uses this for cross file goto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GotoTarget {
    pub path: PathBuf,
    pub span: Span,
}

impl GotoTarget {
    pub fn new(path: PathBuf, span: Span) -> Self {
        Self { path, span }
    }
}

/// Cap for `textDocument/references` results. Bounds fan-out across files.
pub const MAX_REFERENCES: usize = 100;

/// One reference location with owning file.
///
/// `is_declaration` marks the declaration name so dispatch can honor
/// `context.includeDeclaration`. Languages set it via decl spans;
/// usages carry false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceData {
    pub path: PathBuf,
    pub span: Span,
    pub is_declaration: bool,
}

impl ReferenceData {
    pub fn new(path: PathBuf, span: Span, is_declaration: bool) -> Self {
        Self {
            path,
            span,
            is_declaration,
        }
    }
}

/// Request context for position based analysis.
///
/// Carries the current file identity plus the workspace so later work
/// (FORGE-94..96) can resolve cross file lookups from one value.
#[derive(Debug)]
pub struct ProjectContext<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub offset: u32,
    pub workspace: &'a Workspace,
}

impl<'a> ProjectContext<'a> {
    pub fn new(path: &'a Path, text: &'a str, offset: u32, workspace: &'a Workspace) -> Self {
        Self {
            path,
            text,
            offset,
            workspace,
        }
    }

    /// Dependency graph across open files.
    pub fn index(&self) -> &ProjectIndex {
        self.workspace.index()
    }

    /// Latest text for any tracked file.
    pub fn file_text(&self, path: &Path) -> Option<&str> {
        self.workspace.get_text(path)
    }
}

/// Read `path` from tracked docs first, then from disk.
///
/// Disk reads only count when `is_trackable_file` passes, so symlinks
/// and missing files stay out. Returns owned text for both cases.
pub fn read_tracked_or_disk(workspace: &Workspace, path: &Path) -> Option<String> {
    if let Some(text) = workspace.get_text(path) {
        return Some(text.to_string());
    }
    if !is_trackable_file(path) {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Texts of transitive deps for cross file completion.
///
/// Reads tracked files first, then disk only files such as
/// `node_modules` hits. Expands one bounded second hop (A to B on
/// disk to C on disk) from relative imports found in dep texts.
/// Query time only: the index is never mutated. Caps at `cap`
/// entries and stays cycle safe. Depth is two by construction.
pub fn dep_texts(ctx: &ProjectContext, cap: usize) -> Vec<(PathBuf, String)> {
    dep_texts_with(ctx, cap, raw_import_specifiers)
}

fn dep_texts_with(
    ctx: &ProjectContext,
    cap: usize,
    imports_of: fn(&str) -> Vec<String>,
) -> Vec<(PathBuf, String)> {
    if cap == 0 {
        return Vec::new();
    }
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    seen.insert(ctx.path.to_path_buf());
    let mut first: Vec<(PathBuf, String)> = Vec::new();
    for dep in ctx.index().transitive_closure(ctx.path) {
        if out.len() >= cap {
            break;
        }
        if !seen.insert(dep.clone()) {
            continue;
        }
        let Some(text) = read_tracked_or_disk(ctx.workspace, &dep) else {
            continue;
        };
        first.push((dep.clone(), text.clone()));
        out.push((dep, text));
    }
    for (dep_path, dep_text) in &first {
        if out.len() >= cap {
            break;
        }
        for raw in imports_of(dep_text) {
            if out.len() >= cap {
                break;
            }
            let Some(target) = resolve_import(dep_path, &raw) else {
                continue;
            };
            if !seen.insert(target.clone()) {
                continue;
            }
            let Some(text) = read_tracked_or_disk(ctx.workspace, &target) else {
                continue;
            };
            out.push((target, text));
        }
    }
    out
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// Bare import specifiers found by a lexical scan.
///
/// Looks for the word `import` then the next quoted string. This is a
/// query time fallback for disk only deps, not a language grammar:
/// languages keep their own `imports` parsers for indexing.
fn raw_import_specifiers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut cursor = 0;
    while let Some(found) = text[cursor..].find("import") {
        let start = cursor + found;
        let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
        let end = start + "import".len();
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        cursor = end;
        if !before_ok || !after_ok {
            continue;
        }
        let rest = &text[end..];
        let quote_at = rest.find(['"', '\'']);
        let Some(rel) = quote_at else {
            continue;
        };
        let quote = rest.as_bytes()[rel];
        let after_open = &rest[rel + 1..];
        let Some(close_rel) = after_open.find(quote as char) else {
            continue;
        };
        let Some(raw) = after_open.get(..close_rel) else {
            continue;
        };
        // Stop at statement end: a quote search that jumps over `;`
        // or a newline likely grabbed unrelated text.
        let head = &rest[..rel];
        if head.contains(';') || head.contains('\n') {
            continue;
        }
        if !raw.is_empty() {
            out.push(raw.to_string());
        }
        cursor = end + rel + 1 + close_rel + 1;
    }
    out
}
/// Language analyzer: pure analysis behind one trait.
///
/// Core owns this trait and calls it; each `forgels-lang-*` crate
/// implements it. Core never imports language crates, so the binary
/// registers implementations at startup.
pub trait Analyzer: std::fmt::Debug {
    fn hover(&self, ctx: &ProjectContext) -> Option<HoverData>;
    fn complete(&self, ctx: &ProjectContext) -> Vec<CompletionData>;
    /// True when the completion list is truncated and the client must
    /// re-request as the user types instead of filtering locally.
    /// Default is false so stub analyzers keep working.
    fn completion_is_incomplete(&self, _ctx: &ProjectContext) -> bool {
        false
    }
    fn goto(&self, ctx: &ProjectContext) -> Option<Span>;
    fn goto_location(&self, ctx: &ProjectContext) -> Option<GotoTarget> {
        self.goto(ctx)
            .map(|span| GotoTarget::new(ctx.path.to_path_buf(), span))
    }
    fn diagnostics(&self, text: &str) -> Vec<DiagnosticData>;
    /// Context aware diagnostics with dep access. Default keeps single
    /// file behavior so stub analyzers keep working.
    fn diagnostics_ctx(&self, ctx: &ProjectContext) -> Vec<DiagnosticData> {
        self.diagnostics(ctx.text)
    }
    /// Import specifiers in `text`. Default is none so languages
    /// without imports keep working.
    fn imports(&self, _text: &str) -> Vec<ImportRef> {
        Vec::new()
    }
    /// Signature help for the call at the cursor. Default is none so
    /// languages without calls keep working.
    fn signature_help(&self, _ctx: &ProjectContext) -> Option<Vec<SignatureData>> {
        None
    }
    /// Outline entries for the whole file. Default is empty so
    /// languages without symbols keep working.
    fn document_symbols(&self, _text: &str) -> Vec<DocumentSymbolData> {
        Vec::new()
    }
    /// Reference locations for the symbol at the cursor. Default is none
    /// so languages without support keep working.
    fn references(&self, _ctx: &ProjectContext) -> Option<Vec<ReferenceData>> {
        None
    }
    /// Rename edits for the symbol at the cursor. Default is none so
    /// languages without support keep working. Languages validate
    /// `new_name` (ident shape plus keyword guard) and reuse their
    /// reference search for the edit list.
    fn rename(&self, _ctx: &ProjectContext, _new_name: &str) -> Option<Vec<ReferenceData>> {
        None
    }
    /// Inlay hints inside `range`. Default is empty so languages
    /// without support keep working. Takes the full context plus the
    /// requested range so dep lookup works the same as completion.
    fn inlay_hints(&self, _ctx: &ProjectContext, _range: Span) -> Vec<InlayHintData> {
        Vec::new()
    }
    /// Semantic tokens for the whole file. Default is empty so
    /// languages without support keep working. Full only in V1.
    fn semantic_tokens(&self, _text: &str) -> Vec<SemanticTokenData> {
        Vec::new()
    }
    /// Call hierarchy item at the cursor. Default is none so languages
    /// without support keep working.
    fn prepare_call_hierarchy(&self, _ctx: &ProjectContext) -> Option<CallHierarchyItemData> {
        None
    }
    /// Callers of the symbol at the cursor. Default is none.
    fn incoming_calls(&self, _ctx: &ProjectContext) -> Option<Vec<CallHierarchyCallData>> {
        None
    }
    /// Callees of the symbol at the cursor. Default is none.
    fn outgoing_calls(&self, _ctx: &ProjectContext) -> Option<Vec<CallHierarchyCallData>> {
        None
    }
    /// Quickfixes inside `range`. Default is empty so languages
    /// without support keep working. Takes the full context plus the
    /// requested range so dep lookup works the same as completion.
    fn code_actions(&self, _ctx: &ProjectContext, _range: Span) -> Vec<CodeActionData> {
        Vec::new()
    }
    /// Folding ranges for the whole file. Default is empty so
    /// languages without support keep working.
    fn folding_ranges(&self, _text: &str) -> Vec<FoldingRangeData> {
        Vec::new()
    }
    /// Selection chain at the cursor, innermost first. Default is
    /// empty so languages without support keep working. Dispatch
    /// falls back to a zero length range at the cursor.
    fn selection_range(&self, _ctx: &ProjectContext) -> Vec<Span> {
        Vec::new()
    }
    /// Type hierarchy item at the cursor. Default is none so
    /// languages without support keep working.
    fn prepare_type_hierarchy(&self, _ctx: &ProjectContext) -> Option<TypeHierarchyItemData> {
        None
    }
    /// Base contracts of the item at the cursor. Default is none.
    fn supertypes(&self, _ctx: &ProjectContext) -> Option<Vec<TypeHierarchyItemData>> {
        None
    }
    /// Derived contracts of the item at the cursor. Default is none.
    fn subtypes(&self, _ctx: &ProjectContext) -> Option<Vec<TypeHierarchyItemData>> {
        None
    }
}

/// Registered analyzers by language. Empty when the binary wires none.
pub type AnalyzerMap = HashMap<Language, Box<dyn Analyzer>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn open(workspace: &mut Workspace, name: &str, text: &str) {
        workspace
            .open(PathBuf::from(name), Language::Solidity, text.to_string(), 1)
            .expect("open works");
    }

    fn ctx_for<'a>(workspace: &'a Workspace, path: &'a Path, text: &'a str) -> ProjectContext<'a> {
        ProjectContext::new(path, text, 0, workspace)
    }

    #[test]
    fn skips_untracked_and_returns_transitive() {
        let mut workspace = Workspace::new();
        open(&mut workspace, "/A.sol", "a");
        open(&mut workspace, "/B.sol", "b");
        open(&mut workspace, "/C.sol", "c");
        workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
        workspace.set_file_deps(
            PathBuf::from("/B.sol"),
            vec![PathBuf::from("/C.sol"), PathBuf::from("/Missing.sol")],
        );
        let path = PathBuf::from("/A.sol");
        let text = workspace.get_text(&path).expect("tracked").to_string();
        let ctx = ctx_for(&workspace, &path, &text);
        let found = dep_texts(&ctx, 16);
        let names: Vec<String> = found
            .iter()
            .map(|(path, _)| path.to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"/B.sol".to_string()));
        assert!(names.contains(&"/C.sol".to_string()));
        assert!(!names.contains(&"/Missing.sol".to_string()));
    }

    #[test]
    fn caps_results_and_survives_cycle() {
        let mut workspace = Workspace::new();
        open(&mut workspace, "/A.sol", "a");
        open(&mut workspace, "/B.sol", "b");
        workspace.set_file_deps(PathBuf::from("/A.sol"), vec![PathBuf::from("/B.sol")]);
        workspace.set_file_deps(PathBuf::from("/B.sol"), vec![PathBuf::from("/A.sol")]);
        let path = PathBuf::from("/A.sol");
        let text = workspace.get_text(&path).expect("tracked").to_string();
        let ctx = ctx_for(&workspace, &path, &text);
        let capped = dep_texts(&ctx, 1);
        assert_eq!(capped.len(), 1);
        let all = dep_texts(&ctx, 16);
        assert_eq!(all.len(), 1);
    }

    fn fresh_case(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("temp root");
        root
    }

    fn write_file(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(path, text).expect("write");
    }

    #[test]
    fn raw_scan_finds_quoted_specifiers() {
        let text = "import \"./A.sol\";\nimport {B} from './B.sol';\ncontract A {}";
        let found = raw_import_specifiers(text);
        assert_eq!(found, vec!["./A.sol".to_string(), "./B.sol".to_string()]);
        assert!(raw_import_specifiers("contract A {}").is_empty());
    }

    #[test]
    fn disk_only_dep_and_second_hop_load() {
        let root = fresh_case("forgels-dep-disk-hop");
        let a = root.join("contracts/A.sol");
        let b = root.join("node_modules/pkg/B.sol");
        let c = root.join("node_modules/pkg/C.sol");
        write_file(&a, "import \"pkg/B.sol\";");
        write_file(&b, "import \"./C.sol\";\ncontract B {}");
        write_file(&c, "contract C {}");
        let mut workspace = Workspace::new();
        workspace
            .open(
                a.clone(),
                Language::Solidity,
                "import \"pkg/B.sol\";".to_string(),
                1,
            )
            .expect("open works");
        workspace.set_file_deps(a.clone(), vec![b.clone()]);
        let text = workspace.get_text(&a).expect("tracked").to_string();
        let ctx = ctx_for(&workspace, &a, &text);
        let found = dep_texts(&ctx, 16);
        let names: Vec<PathBuf> = found.iter().map(|(path, _)| path.clone()).collect();
        assert!(names.contains(&b));
        assert!(names.contains(&c));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn disk_cycle_and_cap_stay_bounded() {
        let root = fresh_case("forgels-dep-disk-cycle");
        let a = root.join("A.sol");
        let b = root.join("node_modules/pkg/B.sol");
        write_file(&a, "import \"pkg/B.sol\";");
        write_file(&b, "import \"../../A.sol\";\ncontract B {}");
        let mut workspace = Workspace::new();
        workspace
            .open(
                a.clone(),
                Language::Solidity,
                "import \"pkg/B.sol\";".to_string(),
                1,
            )
            .expect("open works");
        workspace.set_file_deps(a.clone(), vec![b.clone()]);
        let text = workspace.get_text(&a).expect("tracked").to_string();
        let ctx = ctx_for(&workspace, &a, &text);
        let found = dep_texts(&ctx, 16);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, b);
        let capped = dep_texts(&ctx, 1);
        assert_eq!(capped.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_tracked_beats_disk() {
        let root = fresh_case("forgels-dep-tracked-first");
        let b = root.join("node_modules/pkg/B.sol");
        write_file(&b, "disk");
        let mut workspace = Workspace::new();
        workspace
            .open(b.clone(), Language::Solidity, "tracked".to_string(), 1)
            .expect("open works");
        assert_eq!(
            read_tracked_or_disk(&workspace, &b).expect("reads"),
            "tracked"
        );
        assert!(read_tracked_or_disk(&workspace, &root.join("Missing.sol")).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
