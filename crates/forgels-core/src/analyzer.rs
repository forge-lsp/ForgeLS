use std::collections::HashMap;

use forgels_syntax::Span;

use crate::routing::Language;

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
}

impl ImportRef {
    pub fn new(raw: String) -> Self {
        Self {
            raw,
            symbols: Vec::new(),
        }
    }

    pub fn with_symbols(raw: String, symbols: Vec<String>) -> Self {
        Self { raw, symbols }
    }
}
/// Language analyzer: pure analysis behind one trait.
///
/// Core owns this trait and calls it; each `forgels-lang-*` crate
/// implements it. Core never imports language crates, so the binary
/// registers implementations at startup.
pub trait Analyzer: std::fmt::Debug {
    fn hover(&self, text: &str, offset: u32) -> Option<HoverData>;
    fn complete(&self, text: &str, offset: u32) -> Vec<CompletionData>;
    fn goto(&self, text: &str, offset: u32) -> Option<Span>;
    fn diagnostics(&self, text: &str) -> Vec<DiagnosticData>;
    /// Import specifiers in `text`. Default is none so languages
    /// without imports keep working.
    fn imports(&self, _text: &str) -> Vec<ImportRef> {
        Vec::new()
    }
}

/// Registered analyzers by language. Empty when the binary wires none.
pub type AnalyzerMap = HashMap<Language, Box<dyn Analyzer>>;
