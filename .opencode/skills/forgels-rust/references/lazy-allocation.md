# Lazy Allocation and Combinators

Scope: `forgels-rust` skill. Avoid paying for values that are never used.

Source: Apollo `book/chapter_01.md` section 1.4.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Prefer `_else` and `_or_else` variants when the fallback allocates or runs work: `unwrap_or_else`, `ok_or_else`, `map_or_else`.
2. Use plain `unwrap_or`, `ok_or`, `map_or` only when the fallback is a cheap constant or already computed.
3. Prefer `unwrap_or_default` over `unwrap_or(Type::new())`.
4. Use `inspect_err` for logging, `map_err` for type conversion. Chain them instead of a logging-only `match`.

## Naming

- Name fallbacks after their role: `fallback_span`, `default_config`.
- Keep closures short. If the closure grows past 3 lines, extract a named function.

## Good vs bad

Good (lazy fallback):

```rust
pub fn config_or_default(raw: Option<Config>) -> Config {
    raw.unwrap_or_else(Config::default)
}

pub fn span_or_zero(raw: Option<Span>) -> Span {
    raw.unwrap_or_default()
}

pub fn parse_with_context(text: &str) -> ParseResult<SolidityFile> {
    parse_text(text)
        .inspect_err(|parse_error| log::warn!("parse failed: {parse_error}"))
        .map_err(|parse_error| ParseError::new(parse_error.span, "solidity parse failed"))
}
```

Bad (eager allocation on the happy path):

```rust
pub fn config_or_default_bad(raw: Option<Config>) -> Config {
    raw.unwrap_or(Config::default())
}

pub fn error_with_eager_text(missing: Option<Span>) -> ParseResult<Span> {
    missing.ok_or(ParseError::new(Span::new(0, 0), format!("missing span at {}", 0)))
}
```

Good fixed form of the second case:

```rust
pub fn error_with_lazy_text(missing: Option<Span>, offset: u32) -> ParseResult<Span> {
    missing.ok_or_else(|| ParseError::new(Span::new(offset, offset), "missing span"))
}
```

## Checklist

1. Fallback allocates only on failure.
2. Logging uses `inspect_err`, conversion uses `map_err`.
3. No eager `String` or `Vec` build before the branch.
