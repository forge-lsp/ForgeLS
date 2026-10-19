//! forgels-fmt: Solidity-only formatter core (P0 trim).
//!
//! P0 owns one rule: trim trailing spaces and tabs plus exactly one
//! trailing newline. Full layout rules land in P1-P3. Format config
//! lives in `forgels-lint::FormatConfig`; this crate defines no config
//! type and takes plain text so the core stays dependency light.

pub mod edit;
pub mod format;
pub mod trim;
pub mod whitespace;

pub use edit::{FormatEdit, FormatOutcome, apply};
pub use format::format_text;
