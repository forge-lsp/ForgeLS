# Generics and Dispatch

Scope: `forgels-rust` skill. Static by default, dynamic only for runtime mix.

Source: Apollo full `book/chapter_06.md`.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Prefer static dispatch (`impl Trait`, `<T: Trait>`) for hot paths and known types. It inlines and keeps errors clear.
2. Use `dyn Trait` only for runtime mix: heterogeneous collections, plugins, stable library seams.
3. Start with generics. Move to `Box<dyn Trait>` when flexibility beats speed with proof.
4. Prefer `&dyn Trait` over `Box<dyn Trait>` when ownership is not needed. Box at the API edge, not deep inside.
5. Share across threads as `Arc<dyn Trait + Send + Sync>`.
6. Keep traits object safe for `dyn`: no generic methods, no `Self: Sized` requirement, only `self`, `&self`, `&mut self` receivers.
7. Never derive `Copy` and `Iterator` on the same type. For a `Copy` iterable shape, impl `IntoIterator` and return a separate iterator struct.

## Naming

- Generic params are single capitals with bounds at use: `fn lex_all<T: Lexer>(lexer: &T, input: &str)`.
- Name the trait by ability: `Lexer`, `Parser`, `Resolver`.
- Name dynamic holders by role: `boxed_parser`, `shared_backend`.

## Good vs bad

Good (static, inlined):

```rust
pub fn lex_all<T: Lexer>(lexer: &T, input: &str) -> Vec<Token> {
    lexer.tokenize(input)
}
```

Good (dynamic only at the seam):

```rust
pub struct Server {
    parsers: Vec<Box<dyn Parser>>,
}
```

Bad (boxed too early, pays vtable for no mix):

```rust
pub struct LexerSlot {
    lexer: Box<dyn Lexer>,
}

pub fn token_count_bad(slot: &LexerSlot, input: &str) -> usize {
    slot.lexer.tokenize(input).len()
}
```

## Checklist

1. Hot path is static, mix point is dynamic.
2. Box lives at the boundary, not in every struct.
3. `dyn` traits are object safe.
