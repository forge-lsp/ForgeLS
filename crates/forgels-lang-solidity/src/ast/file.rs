use super::contract::ContractDef;
use super::{SolidityNode, cast_node, first_ident_text};
use forgels_syntax::{AstNode, Span, SyntaxElement, SyntaxNode};

#[derive(Debug, Clone)]
pub struct SolidityFile {
    syntax: SyntaxNode,
}

impl SolidityFile {
    pub fn new(syntax: SyntaxNode) -> Self {
        Self { syntax }
    }

    fn children_of(&self, kind: SolidityNode) -> Vec<SyntaxNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == kind.as_u16() => Some(node.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn pragmas(&self) -> Vec<PragmaDirective> {
        self.children_of(SolidityNode::Pragma)
            .into_iter()
            .filter_map(PragmaDirective::cast)
            .collect()
    }

    pub fn imports(&self) -> Vec<ImportDirective> {
        self.children_of(SolidityNode::Import)
            .into_iter()
            .filter_map(ImportDirective::cast)
            .collect()
    }

    pub fn contracts(&self) -> Vec<ContractDef> {
        self.children_of(SolidityNode::Contract)
            .into_iter()
            .filter_map(ContractDef::cast)
            .collect()
    }
}

impl AstNode for SolidityFile {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::File) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone)]
pub struct PragmaDirective {
    syntax: SyntaxNode,
}

impl PragmaDirective {
    /// Name token text, for example `solidity`.
    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }
}

impl AstNode for PragmaDirective {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Pragma) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

#[derive(Debug, Clone)]
pub struct ImportDirective {
    syntax: SyntaxNode,
}

impl ImportDirective {
    /// Imported path without quotes, for example `./A.sol`.
    pub fn path(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => {
                    let text = token.text.trim_matches(|mark| mark == '"' || mark == '\'');
                    if token.text.len() != text.len() {
                        Some(text.to_string())
                    } else {
                        None
                    }
                }
                _ => None,
            })
    }

    /// Span of the path token, quotes included, for diagnostics.
    pub fn path_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => {
                    let text = token.text.trim_matches(|mark| mark == '"' || mark == '\'');
                    if token.text.len() != text.len() {
                        Some(token.span)
                    } else {
                        None
                    }
                }
                _ => None,
            })
    }

    /// Imported names and aliases, for example `A` and `B` in `{A as B}`.
    /// Keywords (`as`, `from`) and the path are never stored as children,
    /// so every word child here is a name.
    pub fn symbols(&self) -> Vec<String> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) => {
                    let is_word = token
                        .text
                        .chars()
                        .next()
                        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
                    if is_word {
                        Some(token.text.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect()
    }
}

impl AstNode for ImportDirective {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Import) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
