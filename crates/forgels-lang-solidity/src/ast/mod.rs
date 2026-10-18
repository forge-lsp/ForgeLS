use forgels_syntax::{SyntaxElement, SyntaxNode};

mod calls;
mod contract;
mod emits;
mod file;
mod members;
mod types;
mod user_types;
mod using;

pub use calls::CallStmt;
pub use contract::ContractDef;
pub use emits::EmitStmt;
pub use file::{ImportDirective, PragmaDirective, SolidityFile};
pub use members::{
    FunctionDef, LocalVar, MemberExpr, Mutability, Param, Returns, StateVar, Visibility,
};
pub use types::{BlockNode, TypeNode};
pub use user_types::{EnumDef, ErrorDef, EventDef, ModifierDef, StructDef, UserTypeDef};
pub use using::UsingDirective;

/// Node kinds for the Solidity skeleton. Start at 1000 so token kinds
/// from [`crate::SolidityKind`] never collide.
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolidityNode {
    File = 1000,
    Pragma,
    Import,
    Contract,
    Function,
    Param,
    StateVar,
    LocalVar,
    Type,
    Block,
    Returns,
    Call,
    Struct,
    Enum,
    Member,
    Error,
    UserType,
    Event,
    Modifier,
    Emit,
    Using,
}

impl SolidityNode {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

fn cast_node(node: &SyntaxNode, expected: SolidityNode) -> bool {
    node.kind == expected.as_u16()
}

fn first_ident_text(node: &SyntaxNode) -> Option<String> {
    node.children.iter().find_map(|element| match element {
        SyntaxElement::Token(token) => {
            if is_word_text(&token.text) {
                Some(token.text.clone())
            } else {
                None
            }
        }
        _ => None,
    })
}

fn is_word_text(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
}
