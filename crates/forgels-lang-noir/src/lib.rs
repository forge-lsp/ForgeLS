//! forgels-lang-noir: Noir support.
//!
//! Allowed deps: forgels-core, forgels-protocol, forgels-syntax, forgels-common.
//! Forbidden: any other `forgels-lang-*` crate.

pub mod ast;
pub mod lexer;
pub mod parser;

pub use ast::NoirFile;
pub use lexer::NoirLexer;
pub use parser::NoirParser;

pub const LANGUAGE_ID: &str = "noir";
pub const FILE_EXTENSIONS: &[&str] = &["nr"];
