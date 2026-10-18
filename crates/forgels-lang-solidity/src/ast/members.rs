use super::types::{BlockNode, TypeNode};
use super::{SolidityNode, cast_node, first_ident_text, is_word_text};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, Span, SyntaxElement, SyntaxNode, SyntaxToken};

fn is_literal_token_kind(kind: u16) -> bool {
    kind == SolidityKind::Number.as_u16()
        || kind == SolidityKind::OctalNumber.as_u16()
        || kind == SolidityKind::StringLit.as_u16()
        || kind == SolidityKind::HexString.as_u16()
        || kind == SolidityKind::UnicodeString.as_u16()
        || kind == SolidityKind::True.as_u16()
        || kind == SolidityKind::False.as_u16()
}

fn initializer_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.children.iter().find_map(|element| match element {
        SyntaxElement::Token(token) if is_literal_token_kind(token.kind) => Some(token.clone()),
        _ => None,
    })
}

/// Data location for reference types: `memory`, `storage`, `calldata`.
///
/// Stored as a token child in `Param`, `StateVar`, and `LocalVar` between
/// the type node and the name. Value types never carry one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataLocation {
    Memory,
    Storage,
    Calldata,
}

impl DataLocation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Storage => "storage",
            Self::Calldata => "calldata",
        }
    }

    pub fn from_kind(kind: u16) -> Option<Self> {
        if kind == SolidityKind::Memory.as_u16() {
            Some(Self::Memory)
        } else if kind == SolidityKind::Storage.as_u16() {
            Some(Self::Storage)
        } else if kind == SolidityKind::Calldata.as_u16() {
            Some(Self::Calldata)
        } else {
            None
        }
    }
}

pub fn is_data_location_kind(kind: u16) -> bool {
    DataLocation::from_kind(kind).is_some()
}

/// Visibility modifier for functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Internal,
    External,
}

/// State mutability modifier for functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    Pure,
    View,
    Payable,
}

/// Function definition: name, params, return types, and body block.
///
/// Children hold the name token, Param nodes, visibility and mutability
/// keyword tokens, `virtual` and `override` tokens with override bases,
/// one Returns node, then one Block node. Custom modifiers stay skipped.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    syntax: SyntaxNode,
}

impl FunctionDef {
    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }

    pub fn name_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
    }

    pub fn params(&self) -> Vec<Param> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Param.as_u16() => {
                    Param::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn return_types(&self) -> Vec<TypeNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Returns.as_u16() => {
                    Returns::cast(node.clone())
                }
                _ => None,
            })
            .flat_map(|returns| returns.types())
            .collect()
    }

    pub fn body(&self) -> Option<BlockNode> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Block.as_u16() => {
                    BlockNode::cast(node.clone())
                }
                _ => None,
            })
    }

    pub fn visibility(&self) -> Option<Visibility> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => match token.kind {
                    kind if kind == SolidityKind::Public.as_u16() => Some(Visibility::Public),
                    kind if kind == SolidityKind::Private.as_u16() => Some(Visibility::Private),
                    kind if kind == SolidityKind::Internal.as_u16() => Some(Visibility::Internal),
                    kind if kind == SolidityKind::External.as_u16() => Some(Visibility::External),
                    _ => None,
                },
                _ => None,
            })
    }

    pub fn visibility_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => match token.kind {
                    kind if kind == SolidityKind::Public.as_u16()
                        || kind == SolidityKind::Private.as_u16()
                        || kind == SolidityKind::Internal.as_u16()
                        || kind == SolidityKind::External.as_u16() =>
                    {
                        Some(token.span)
                    }
                    _ => None,
                },
                _ => None,
            })
    }

    pub fn mutability(&self) -> Option<Mutability> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => match token.kind {
                    kind if kind == SolidityKind::Pure.as_u16() => Some(Mutability::Pure),
                    kind if kind == SolidityKind::View.as_u16()
                        || kind == SolidityKind::Constant.as_u16() =>
                    {
                        Some(Mutability::View)
                    }
                    kind if kind == SolidityKind::Payable.as_u16() => Some(Mutability::Payable),
                    _ => None,
                },
                _ => None,
            })
    }

    pub fn mutability_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => match token.kind {
                    kind if kind == SolidityKind::Pure.as_u16()
                        || kind == SolidityKind::View.as_u16()
                        || kind == SolidityKind::Payable.as_u16()
                        || kind == SolidityKind::Constant.as_u16() =>
                    {
                        Some(token.span)
                    }
                    _ => None,
                },
                _ => None,
            })
    }

    pub fn is_virtual(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Virtual.as_u16()
            )
        })
    }

    pub fn is_override(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Override.as_u16()
            )
        })
    }

    pub fn virtual_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Virtual.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
    }

    pub fn override_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Override.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
    }

    pub fn override_bases(&self) -> Vec<String> {
        let mut at_override = false;
        let mut in_list = false;
        let mut out = Vec::new();
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Override.as_u16() => {
                    at_override = true;
                }
                SyntaxElement::Token(token)
                    if at_override && token.kind == SolidityKind::LParen.as_u16() =>
                {
                    in_list = true;
                }
                SyntaxElement::Token(token)
                    if at_override && token.kind == SolidityKind::RParen.as_u16() =>
                {
                    break;
                }
                SyntaxElement::Token(token)
                    if in_list && token.kind == SolidityKind::Ident.as_u16() =>
                {
                    out.push(token.text.clone());
                }
                _ => {}
            }
        }
        out
    }
}

impl AstNode for FunctionDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Function) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// One function or returns-clause parameter: type plus optional name.
#[derive(Debug, Clone)]
pub struct Param {
    syntax: SyntaxNode,
}

impl Param {
    pub fn type_node(&self) -> Option<TypeNode> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Type.as_u16() => {
                    TypeNode::cast(node.clone())
                }
                _ => None,
            })
    }

    pub fn data_location(&self) -> Option<DataLocation> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => DataLocation::from_kind(token.kind),
                _ => None,
            })
    }

    pub fn data_location_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if is_data_location_kind(token.kind) => {
                    Some(token.span)
                }
                _ => None,
            })
    }

    pub fn name(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token)
                    if !is_data_location_kind(token.kind) && is_word_text(&token.text) =>
                {
                    Some(token.text.clone())
                }
                _ => None,
            })
    }

    pub fn name_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token)
                    if !is_data_location_kind(token.kind) && is_word_text(&token.text) =>
                {
                    Some(token.span)
                }
                _ => None,
            })
    }
}

impl AstNode for Param {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Param) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn is_var_modifier_kind(kind: u16) -> bool {
    kind == SolidityKind::Public.as_u16()
        || kind == SolidityKind::Virtual.as_u16()
        || kind == SolidityKind::Override.as_u16()
}

fn var_name_and_type(node: &SyntaxNode) -> (Option<TypeNode>, Option<String>) {
    let mut type_node = None;
    let mut name = None;
    for element in &node.children {
        match element {
            SyntaxElement::Node(child) if child.kind == SolidityNode::Type.as_u16() => {
                if type_node.is_none() {
                    type_node = TypeNode::cast(child.clone());
                }
            }
            SyntaxElement::Token(token)
                if name.is_none()
                    && !is_var_modifier_kind(token.kind)
                    && !is_data_location_kind(token.kind)
                    && is_word_text(&token.text) =>
            {
                name = Some(token.text.clone());
            }
            _ => {}
        }
    }
    (type_node, name)
}

fn var_location(node: &SyntaxNode) -> Option<DataLocation> {
    node.children.iter().find_map(|element| match element {
        SyntaxElement::Token(token) => DataLocation::from_kind(token.kind),
        _ => None,
    })
}

fn var_location_span(node: &SyntaxNode) -> Option<Span> {
    node.children.iter().find_map(|element| match element {
        SyntaxElement::Token(token) if is_data_location_kind(token.kind) => Some(token.span),
        _ => None,
    })
}

/// Returns clause: the parenthesized type list after `returns`.
/// Children hold `Param` nodes; bare types never appear here directly.
#[derive(Debug, Clone)]
pub struct Returns {
    syntax: SyntaxNode,
}

impl Returns {
    pub fn types(&self) -> Vec<TypeNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(child) if child.kind == SolidityNode::Param.as_u16() => {
                    Param::cast(child.clone()).and_then(|param| param.type_node())
                }
                _ => None,
            })
            .collect()
    }
}

impl AstNode for Returns {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Returns) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// State variable: type plus name. The node span covers any initializer.
/// Children hold the type node, the name token, plus one literal token
/// when the initializer is exactly one literal.
#[derive(Debug, Clone)]
pub struct StateVar {
    syntax: SyntaxNode,
}

impl StateVar {
    pub fn type_node(&self) -> Option<TypeNode> {
        var_name_and_type(&self.syntax).0
    }

    pub fn name(&self) -> Option<String> {
        var_name_and_type(&self.syntax).1
    }

    pub fn data_location(&self) -> Option<DataLocation> {
        var_location(&self.syntax)
    }

    pub fn data_location_span(&self) -> Option<Span> {
        var_location_span(&self.syntax)
    }

    pub fn name_span(&self) -> Option<Span> {
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Node(child) if child.kind == SolidityNode::Type.as_u16() => {}
                SyntaxElement::Token(token)
                    if !is_var_modifier_kind(token.kind)
                        && !is_data_location_kind(token.kind)
                        && is_word_text(&token.text) =>
                {
                    return Some(token.span);
                }
                _ => {}
            }
        }
        None
    }

    pub fn initializer(&self) -> Option<SyntaxToken> {
        initializer_token(&self.syntax)
    }

    pub fn initializer_span(&self) -> Option<Span> {
        self.initializer().map(|token| token.span)
    }

    pub fn is_public(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Public.as_u16()
            )
        })
    }

    pub fn is_virtual(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Virtual.as_u16()
            )
        })
    }

    pub fn is_override(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Override.as_u16()
            )
        })
    }

    pub fn visibility_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Public.as_u16()
                        || token.kind == SolidityKind::Private.as_u16()
                        || token.kind == SolidityKind::Internal.as_u16() =>
                {
                    Some(token.span)
                }
                _ => None,
            })
    }
}

impl AstNode for StateVar {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::StateVar) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Local variable: same shape as StateVar, found inside blocks.
#[derive(Debug, Clone)]
pub struct LocalVar {
    syntax: SyntaxNode,
}

impl LocalVar {
    pub fn type_node(&self) -> Option<TypeNode> {
        var_name_and_type(&self.syntax).0
    }

    pub fn name(&self) -> Option<String> {
        var_name_and_type(&self.syntax).1
    }

    pub fn data_location(&self) -> Option<DataLocation> {
        var_location(&self.syntax)
    }

    pub fn data_location_span(&self) -> Option<Span> {
        var_location_span(&self.syntax)
    }

    pub fn name_span(&self) -> Option<Span> {
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Node(child) if child.kind == SolidityNode::Type.as_u16() => {}
                SyntaxElement::Token(token)
                    if !is_var_modifier_kind(token.kind)
                        && !is_data_location_kind(token.kind)
                        && is_word_text(&token.text) =>
                {
                    return Some(token.span);
                }
                _ => {}
            }
        }
        None
    }

    pub fn initializer(&self) -> Option<SyntaxToken> {
        initializer_token(&self.syntax)
    }

    pub fn initializer_span(&self) -> Option<Span> {
        self.initializer().map(|token| token.span)
    }
}

impl AstNode for LocalVar {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::LocalVar) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Member access with single ident receiver: `p.age`, `this.total`.
///
/// Children hold receiver token, dot token, then optional member token.
/// Only one dot is stored; chained `a.b.c` keeps the first pair and the
/// rest is skipped as statement tail. Call options `{value: 1}` add `{`
/// and `}` markers after the member when followed by parens. Calls add
/// markers after that: `(` plus one `,` per extra arg for non-empty, `)`
/// alone for empty. Plain access holds no marker, so `is_call` separates
/// reads from calls.
#[derive(Debug, Clone)]
pub struct MemberExpr {
    syntax: SyntaxNode,
}

impl MemberExpr {
    pub fn receiver(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => Some(token.text.clone()),
                _ => None,
            })
    }

    pub fn member(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) => Some(token.text.clone()),
                _ => None,
            })
            .nth(2)
    }

    pub fn member_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) => Some(token.span),
                _ => None,
            })
            .nth(2)
    }

    pub fn is_call(&self) -> bool {
        self.syntax.children.iter().skip(3).any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::LParen.as_u16()
                        || token.kind == SolidityKind::Comma.as_u16()
                        || token.kind == SolidityKind::RParen.as_u16()
            )
        })
    }

    pub fn arg_count(&self) -> usize {
        self.syntax
            .children
            .iter()
            .skip(3)
            .filter(|element| {
                matches!(
                    element,
                    SyntaxElement::Token(token)
                        if token.kind == SolidityKind::LParen.as_u16()
                            || token.kind == SolidityKind::Comma.as_u16()
                )
            })
            .count()
    }

    pub fn has_call_options(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::LBrace.as_u16()
            )
        })
    }

    pub fn options_span(&self) -> Option<Span> {
        let mut open = None;
        let mut close = None;
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::LBrace.as_u16() => {
                    open = Some(token.span);
                }
                SyntaxElement::Token(token) if token.kind == SolidityKind::RBrace.as_u16() => {
                    close = Some(token.span);
                }
                _ => {}
            }
        }
        match (open, close) {
            (Some(start), Some(end)) => Some(Span::new(start.start, end.end)),
            (Some(start), None) => Some(start),
            _ => None,
        }
    }
}

impl AstNode for MemberExpr {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Member) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
