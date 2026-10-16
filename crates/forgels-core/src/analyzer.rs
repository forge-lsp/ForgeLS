use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use forgels_syntax::Span;

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

/// Texts of transitive deps for cross file completion.
///
/// Reads tracked files only, skips untracked, dedups by path,
/// and caps at `cap` entries. Cycle safe via the index closure.
pub fn dep_texts(ctx: &ProjectContext, cap: usize) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for dep in ctx.index().transitive_closure(ctx.path) {
        if out.len() >= cap {
            break;
        }
        if !seen.insert(dep.clone()) {
            continue;
        }
        let Some(text) = ctx.file_text(&dep) else {
            continue;
        };
        out.push((dep, text.to_string()));
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
    /// Import specifiers in `text`. Default is none so languages
    /// without imports keep working.
    fn imports(&self, _text: &str) -> Vec<ImportRef> {
        Vec::new()
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
}
