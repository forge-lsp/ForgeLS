//! forgels-protocol: LSP and JSON-RPC types.
//!
//! Keep protocol definitions here. No language logic allowed.

pub mod lsp;
pub mod rpc;

pub use lsp::{
    CompletionItemKind, Diagnostic, DiagnosticSeverity, InlayHintKind, Location, Position, Range,
    SEMANTIC_TOKEN_MODIFIERS, SEMANTIC_TOKEN_TYPES, SymbolKind,
};
pub use rpc::{Request, Response};
