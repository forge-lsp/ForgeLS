use super::members::{FunctionDef, StateVar};
use super::user_types::{EnumDef, ErrorDef, StructDef, UserTypeDef};
use super::{SolidityNode, cast_node};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, SyntaxElement, SyntaxNode};

#[derive(Debug, Clone)]
pub struct ContractDef {
    syntax: SyntaxNode,
}

impl ContractDef {
    /// Contract name: the first identifier token, for example `C`.
    /// Keyword tokens such as `abstract` never match, so stored
    /// modifiers cannot shadow the name.
    pub fn name(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.text.clone())
                }
                _ => None,
            })
    }

    /// Base contract names from the `is` list, for example `A, B`.
    /// Empty when the contract has no heritage.
    pub fn heritage(&self) -> Vec<String> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.text.clone())
                }
                _ => None,
            })
            .skip(1)
            .collect()
    }

    /// True when declared `abstract contract`.
    pub fn is_abstract(&self) -> bool {
        self.syntax.children.iter().any(|element| match element {
            SyntaxElement::Token(token) => token.kind == SolidityKind::Abstract.as_u16(),
            _ => false,
        })
    }

    fn members_of(&self, kind: SolidityNode) -> Vec<SyntaxNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == kind.as_u16() => Some(node.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn functions(&self) -> Vec<FunctionDef> {
        self.members_of(SolidityNode::Function)
            .into_iter()
            .filter_map(FunctionDef::cast)
            .collect()
    }

    pub fn state_vars(&self) -> Vec<StateVar> {
        self.members_of(SolidityNode::StateVar)
            .into_iter()
            .filter_map(StateVar::cast)
            .collect()
    }

    pub fn structs(&self) -> Vec<StructDef> {
        self.members_of(SolidityNode::Struct)
            .into_iter()
            .filter_map(StructDef::cast)
            .collect()
    }

    pub fn enums(&self) -> Vec<EnumDef> {
        self.members_of(SolidityNode::Enum)
            .into_iter()
            .filter_map(EnumDef::cast)
            .collect()
    }

    pub fn errors(&self) -> Vec<ErrorDef> {
        self.members_of(SolidityNode::Error)
            .into_iter()
            .filter_map(ErrorDef::cast)
            .collect()
    }

    pub fn user_types(&self) -> Vec<UserTypeDef> {
        self.members_of(SolidityNode::UserType)
            .into_iter()
            .filter_map(UserTypeDef::cast)
            .collect()
    }
}

impl AstNode for ContractDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Contract) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
