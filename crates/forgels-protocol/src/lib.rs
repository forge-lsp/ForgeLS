//! forgels-protocol: LSP and JSON-RPC types.
//!
//! Keep protocol definitions here. No language logic allowed.

pub mod lsp;
pub mod rpc;

pub use lsp::{
    CompletionItemKind, Diagnostic, DiagnosticSeverity, Location, Position, Range, SymbolKind,
};
pub use rpc::{Request, Response};
