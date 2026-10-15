use forgels_syntax::{AstNode, SyntaxElement, SyntaxNode};

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
}

impl SolidityNode {
    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

fn cast_node(node: &SyntaxNode, expected: SolidityNode) -> bool {
    node.kind == expected.as_u16()
}

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

#[derive(Debug, Clone)]
pub struct ContractDef {
    syntax: SyntaxNode,
}

impl ContractDef {
    /// Contract name, for example `C`.
    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
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

/// Function definition: name, params, return types, and body block.
///
/// Children hold the name token, Param nodes, return Type nodes, then one
/// Block node. Modifier keywords are consumed but not stored; the node
/// span still covers them.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    syntax: SyntaxNode,
}

impl FunctionDef {
    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
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
        let mut types = Vec::new();
        for element in &self.syntax.children {
            let SyntaxElement::Node(node) = element else {
                continue;
            };
            if node.kind == SolidityNode::Type.as_u16() {
                types.extend(TypeNode::cast(node.clone()));
            } else if node.kind == SolidityNode::Returns.as_u16() {
                types.extend(returns_types(node));
            }
        }
        types
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

    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
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
            SyntaxElement::Token(token) if name.is_none() && is_word_text(&token.text) => {
                name = Some(token.text.clone());
            }
            _ => {}
        }
    }
    (type_node, name)
}

/// State variable: type plus name. The node span covers any initializer.
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

/// Type reference: elementary, mapping, sized, array, or user type.
///
/// Children hold the raw tokens. Type text joins them with minimal
/// spacing so hover shows mapping(address => uint).
#[derive(Debug, Clone)]
pub struct TypeNode {
    syntax: SyntaxNode,
}

impl TypeNode {
    pub fn text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        collect_token_text(&self.syntax, &mut parts);
        join_type_text(&parts)
    }
}

fn collect_token_text<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a str>) {
    for element in &node.children {
        match element {
            SyntaxElement::Token(token) => out.push(token.text.as_str()),
            SyntaxElement::Node(child) => collect_token_text(child, out),
        }
    }
}

fn returns_types(node: &SyntaxNode) -> Vec<TypeNode> {
    let mut types = Vec::new();
    for element in &node.children {
        let SyntaxElement::Node(child) = element else {
            continue;
        };
        if child.kind == SolidityNode::Type.as_u16() {
            types.extend(TypeNode::cast(child.clone()));
        } else if child.kind == SolidityNode::Param.as_u16()
            && let Some(param) = Param::cast(child.clone())
            && let Some(type_node) = param.type_node()
        {
            types.push(type_node);
        }
    }
    types
}

impl AstNode for TypeNode {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Type) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn join_type_text(parts: &[&str]) -> String {
    // Explicit spacing: no guesswork, no chained replaces.
    // A token takes no leading space when it clings to the previous one
    // (`(`, `[`, `)`, `]`, `,`, `;`, `.`), or when the previous token
    // opens a group (`(`, `[`, `.`). Everything else separates with one
    // space, which gives `=>` its surrounding spaces for free.
    const NO_SPACE_BEFORE: &[&str] = &["(", "[", ")", "]", ",", ";", "."];
    const NO_SPACE_AFTER: &[&str] = &["(", "[", "."];
    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0
            && !NO_SPACE_BEFORE.contains(part)
            && !NO_SPACE_AFTER.contains(&parts[index - 1])
        {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

/// Function or lone block body. Children hold LocalVar nodes only;
/// other statements are skipped but stay inside the node span.
#[derive(Debug, Clone)]
pub struct BlockNode {
    syntax: SyntaxNode,
}

impl BlockNode {
    pub fn locals(&self) -> Vec<LocalVar> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::LocalVar.as_u16() => {
                    LocalVar::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }
}

impl AstNode for BlockNode {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Block) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
