//! Solidity lexer: token kinds and byte scanner.

mod kinds;
mod scan;

#[cfg(test)]
mod tests;

pub use kinds::{SolidityKind, keyword_kind, symbol_kind};
pub use scan::SolidityLexer;
