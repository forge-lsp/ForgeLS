//! forgels-core: server lifecycle, workspace manager, config, diagnostics hub.
//!
//! This crate owns cross language orchestration. It must not contain
//! language specific grammar. It dispatches to `forgels-lang-*` crates.

pub mod analyzer;
pub mod config;
pub mod diagnostics;
pub mod dispatch;
pub mod index;
pub mod routing;
pub mod server;
pub mod transport;
pub mod workspace;

pub use analyzer::{
    Analyzer, AnalyzerMap, CallHierarchyCallData, CallHierarchyItemData, CodeActionData,
    CompletionData, DiagnosticData, DocumentSymbolData, FoldingRangeData, GotoTarget, HoverData,
    ImportRef, InlayHintData, MAX_CALL_HIERARCHY, MAX_CODE_ACTIONS, MAX_FOLDING_RANGES,
    MAX_INLAY_HINTS, MAX_REFERENCES, MAX_SELECTION_DEPTH, MAX_SEMANTIC_TOKENS, MAX_TYPE_HIERARCHY,
    MAX_WORKSPACE_SYMBOLS, ProjectContext, ReferenceData, SemanticTokenData, SignatureData,
    SignatureParam, TextEdit, TypeHierarchyItemData, WorkspaceSymbolData, dep_texts,
    read_tracked_or_disk, workspace_symbols,
};
pub use config::Config;
pub use index::{FileEntry, ProjectIndex};
pub use server::Server;
pub use workspace::Workspace;
