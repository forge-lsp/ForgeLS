use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use forgels_protocol::CompletionItemKind;
use forgels_syntax::Span;

use forgels_common::fs::is_trackable_file;
use forgels_common::paths::resolve_import;

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

/// One parse problem from a language analyzer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticData {
    pub span: Span,
    pub message: String,
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
