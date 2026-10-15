# Naming: Variables, Functions, Types

Scope: `forgels-rust` skill. Names carry the design so comments stay rare.

Source: Rust API Guidelines on naming, https://rust-lang.github.io/api-guidelines/naming.html, plus Apollo naming habits across `book/chapter_01.md` to `book/chapter_09.md`.
Upstream book: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Use `snake_case` for variables, functions, and modules. Use `UpperCamelCase` for types, traits, and enum variants. Use `SCREAMING_SNAKE_CASE` for constants and statics.
2. Name variables after their role, not their type: `span`, `token`, `symbol_table`. Never `data`, `thing`, `tmp`, `val1`.
3. Name bools as predicates: `is_valid`, `has_span`, `can_hover`. Never `flag`, `check`, `status_bool`.
4. Name functions as verb plus object: `parse_text`, `resolve_symbol`, `load_config`. Never `do_stuff`, `handle`, `process_data`.
5. Name fallible helpers by outcome: `parse_or_default`, `span_or_zero`. Avoid `get`, `fetch_data`, `do_parse`.
6. Keep acronyms consistent: one style per codebase. Prefer `span`, `ast`, `lsp` in lowercase inside snake names (`span_of`, `ast_node`), not mixed `SpanOf` or `ASTNode` in locals.
7. Avoid redundant prefixes: `token_span` is enough inside a token module, not `token_token_span`.
8. Count with nouns: `token_count`, `symbol_len`. Name indexes `token_index`, never `i` past a 3-line loop.
9. Keep lifetimes and generics short but honest: `'a`, `T`, `E`. Long names only when they add meaning (`State`, `Backend`).
10. Match ForgeLS vocabulary: `Span`, `Token`, `ParseError`, `SyntaxNode`, `Lexer::tokenize`, `Parser::parse_text`.

## Good vs bad

Good (role names, verb plus object):

```rust
pub fn resolve_symbol(symbols: &[Symbol], name: &str) -> Option<&Symbol> {
    let wanted = name.trim();
    symbols.iter().find(|symbol| symbol.name == wanted)
}

pub fn span_or_zero(maybe_span: Option<Span>) -> Span {
    maybe_span.unwrap_or_default()
}
```

Bad (vague names, weak verbs):

```rust
pub fn handle(d: Vec<Symbol>, s: String) -> Option<Symbol> {
    let t = s.clone();
    d.into_iter().find(|e| e.name == t)
}
```

Good (predicate and count names):

```rust
pub fn has_valid_header(text: &str) -> bool {
    text.trim_start().starts_with("contract")
}

pub fn token_count(tokens: &[Token]) -> usize {
    tokens.len()
}
```

Bad (bool as noun, index as `i` in long loop):

```rust
pub fn check(t: &str) -> bool {
    t.starts_with("c")
}
```

## Checklist

1. Every name states a role or an action.
2. No `data`, `thing`, `tmp`, `handle`, `process_data`.
3. Casing follows Rust conventions without mix.
