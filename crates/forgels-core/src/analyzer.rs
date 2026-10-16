use std::collections::HashMap;
use std::path::Path;

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
/// Language analyzer: pure analysis behind one trait.
///
/// Core owns this trait and calls it; each `forgels-lang-*` crate
/// implements it. Core never imports language crates, so the binary
/// registers implementations at startup.
pub trait Analyzer: std::fmt::Debug {
    fn hover(&self, ctx: &ProjectContext) -> Option<HoverData>;
    fn complete(&self, ctx: &ProjectContext) -> Vec<CompletionData>;
    fn goto(&self, ctx: &ProjectContext) -> Option<Span>;
    fn diagnostics(&self, text: &str) -> Vec<DiagnosticData>;
    /// Import specifiers in `text`. Default is none so languages
    /// without imports keep working.
    fn imports(&self, _text: &str) -> Vec<ImportRef> {
        Vec::new()
    }
}

/// Registered analyzers by language. Empty when the binary wires none.
pub type AnalyzerMap = HashMap<Language, Box<dyn Analyzer>>;
