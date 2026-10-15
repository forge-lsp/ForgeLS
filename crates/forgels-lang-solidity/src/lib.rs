//! forgels-lang-solidity: Solidity support.
//!
//! Allowed deps: forgels-core, forgels-protocol, forgels-syntax, forgels-common.
//! Forbidden: any other `forgels-lang-*` crate.

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::SolidityFile;
pub use lexer::SolidityLexer;
pub use parser::SolidityParser;

pub const LANGUAGE_ID: &str = "solidity";
pub const FILE_EXTENSIONS: &[&str] = &["sol"];
