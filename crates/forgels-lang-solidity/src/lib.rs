//! forgels-lang-solidity: Solidity support.
//!
//! Allowed deps: forgels-core, forgels-protocol, forgels-syntax, forgels-common.
//! Forbidden: any other `forgels-lang-*` crate.

pub mod analysis;
pub mod ast;
pub mod lexer;
pub mod parser;

pub use analysis::{
    CompletionItem, CompletionKind, Hover, Scope, SolidityAnalyzer, Symbol, SymbolKind,
    collect_symbols, complete_at, complete_import_paths, completion_item_kind, exports_of,
    goto_definition, hover_at, import_path_at, imports_in, resolve_in_dep,
};

pub use ast::{
    BlockNode, CallStmt, ContractDef, EmitStmt, EnumDef, ErrorDef, EventDef, FunctionDef,
    ImportDirective, LocalVar, MemberExpr, ModifierDef, Param, PragmaDirective, Returns,
    SolidityFile, SolidityNode, StateVar, StructDef, TypeNode, UserTypeDef,
};
pub use lexer::{KEYWORDS, SolidityKind, SolidityLexer, keyword_kind, symbol_kind};
pub use parser::SolidityParser;

pub const LANGUAGE_ID: &str = "solidity";
pub const FILE_EXTENSIONS: &[&str] = &["sol"];
