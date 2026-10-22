//! forgels-fmt: Solidity-only formatter core.
//!
//! Owns trim plus token-gap spacing plus call wrapping plus blank
//! lines plus brace-depth indent. Width and indent come from
//! `FormatOptions` (default 120 and 4 spaces). Format config lives in
//! `forgels-lint::FormatConfig`; this crate takes plain options so
//! the core stays dependency light.

pub mod blank;
pub mod edit;
pub mod format;
pub mod indent;
pub mod options;
pub mod trim;
pub mod whitespace;
pub mod wrap;

pub use edit::{FormatEdit, FormatOutcome, apply};
pub use format::{comment_spans, format_text};
pub use options::{FormatOptions, IndentStyle};
