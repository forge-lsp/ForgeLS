//! forgels-protocol: LSP and JSON-RPC types.
//!
//! Keep protocol definitions here. No language logic allowed.

pub mod lsp;
pub mod rpc;

pub use lsp::{Diagnostic, DiagnosticSeverity, Location, Position, Range};
pub use rpc::{Request, Response};
