//! forgels-syntax: shared AST, lexer, and parser toolkit.
//!
//! Purpose: reusable foundation used by every `forgels-lang-*` crate.
//! Keep this crate language agnostic. No Solidity, Noir, or other
//! language specific grammar here. Each language defines its own
//! SyntaxKind, AST nodes, and parser on top of these traits.

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod span;
pub mod syntax;

pub use ast::{AstNode, Visitor};
pub use error::{ParseError, ParseResult};
pub use lexer::{Lexer, Token};
pub use parser::Parser;
pub use span::Span;
pub use syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
