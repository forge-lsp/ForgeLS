//! forgels-fmt: Solidity-only formatter core (V1 trim plus space plus wrap).
//!
//! V1 owns trim plus token-gap spacing plus 120-column call wrapping.
//! No blank or indent engine and no config keys: width 120 and indent
//! 4 are fixed. Format config lives in `forgels-lint::FormatConfig`;
//! this crate defines no config type and takes plain text so the core
//! stays dependency light.

pub mod edit;
pub mod format;
pub mod trim;
pub mod whitespace;
pub mod wrap;

pub use edit::{FormatEdit, FormatOutcome, apply};
pub use format::format_text;
