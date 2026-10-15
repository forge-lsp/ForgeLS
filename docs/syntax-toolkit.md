# Syntax Toolkit

Scope: `forgels-syntax` crate. Shared AST and parser base for all languages.

## Provides

- `Span`: start and end offsets. Required for diagnostics.
- `Token`: kind, text, span.
- `Lexer` trait: `tokenize(&self, input: &str) -> Vec<Token>`.
- `Parser` trait: `parse(&self, tokens: &[Token])` plus `parse_text`.
- `SyntaxNode`, `SyntaxToken`, `SyntaxElement`: untyped tree.
- `AstNode` trait: typed wrapper with `cast` and `syntax`.
- `Visitor` trait: common traversal hook.
- `ParseError` and `ParseResult`: error with span and message.

## Use per language

1. Define language `SyntaxKind` values.
2. Implement `Lexer` for the language.
3. Implement `Parser` and return a typed file node.
4. Wrap results with `AstNode`.

Do not add language grammar here. Keep this crate agnostic.
