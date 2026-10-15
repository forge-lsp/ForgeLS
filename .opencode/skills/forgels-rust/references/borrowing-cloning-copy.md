# Borrowing, Cloning, and Copy

Scope: `forgels-rust` skill. Ownership choices for ForgeLS parser and analysis code.

Source: Apollo `book/chapter_01.md` sections 1.1-1.2, `book/chapter_03.md` sections 3.2-3.3.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Borrow by default. Take `&str` over `String`, `&[Token]` over `Vec<Token>`, `&Span` over `Span` when read only.
2. Clone only to keep an owned snapshot while the original must live on. If a function clones its own `&` argument, change the signature to take ownership and push the clone to the caller.
3. Never clone inside a loop to feed the next step. Use `.cloned()`, `.copied()`, or restructure to borrow.
4. Pass by value only when the type is `Copy` and small (up to about 24 bytes, all fields `Copy`, no heap use).
5. Derive `Copy` only for plain data without heap fields. Never put `Copy` and `Iterator` on the same type.

## Naming

- Function names state the borrow contract: `lex_text`, `find_symbol`, `span_of`.
- Avoid vague names: `data`, `thing`, `tmp`, `do_it`.
- Use `input` for borrowed text, `tokens` for slices, `owned_text` only when ownership is real.

## Good vs bad

Good (borrow, no clone):

```rust
pub fn token_count(tokens: &[Token]) -> usize {
    tokens.len()
}

pub fn lookup_symbol(symbols: &[Symbol], name: &str) -> Option<&Symbol> {
    symbols.iter().find(|entry| entry.name == name)
}
```

Bad (needless owned values and clone):

```rust
pub fn token_count_bad(tokens: Vec<Token>) -> usize {
    let owned = tokens.clone();
    owned.len()
}

pub fn lookup_symbol_bad(symbols: Vec<Symbol>, name: String) -> Option<Symbol> {
    symbols.into_iter().find(|entry| entry.name == name.clone())
}
```

Good (small `Copy` type passed by value):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

pub fn span_len(span: Span) -> u32 {
    span.end.saturating_sub(span.start)
}
```

Bad (heap type marked `Copy`, does not compile for a reason):

```rust
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
}
```

## Checklist

1. Signature borrows unless ownership is required.
2. No `.clone()` inside loops or on `&` arguments.
3. `Copy` only on small heap-free types.
