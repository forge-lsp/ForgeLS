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
    Analyzer, AnalyzerMap, CompletionData, DiagnosticData, DocumentSymbolData, GotoTarget,
    HoverData, ImportRef, MAX_REFERENCES, MAX_WORKSPACE_SYMBOLS, ProjectContext, ReferenceData,
    SignatureData, SignatureParam, TextEdit, WorkspaceSymbolData, dep_texts, read_tracked_or_disk,
    workspace_symbols,
};
pub use config::Config;
pub use index::{FileEntry, ProjectIndex};
pub use server::Server;
pub use workspace::Workspace;
