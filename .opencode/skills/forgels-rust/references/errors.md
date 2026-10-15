# Error Design with thiserror and anyhow

Scope: `forgels-rust` skill. Typed errors in crates, ergonomic errors in the binary.

Source: Apollo full `book/chapter_04.md`.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Fallible functions return `Result<T, E>`. Panic only for bugs, tests, or explicit crash paths.
2. Library and language crates use `thiserror` enums. Derive `From` with `#[from]` for layered errors.
3. The binary may use `anyhow` with `.context()`. Never expose `anyhow::Result` from a library API.
4. Bubble with `?`. Log with `inspect_err`, convert with `map_err`. Avoid match pyramids.
5. ForgeLS map: convert every `ParseError` into `forgels_protocol::Diagnostic` at the server edge. No raw parser error reaches the editor.
6. Async errors that cross tasks must be `Send + Sync + 'static`.
7. Test errors by message or value. Assert `to_string()` when the type has no `PartialEq`.

## Naming

- Error types are nouns: `ParseError`, `ConfigError`, `ServerError`.
- Variants state the failure: `EmptyInput`, `MissingSpan`, `InvalidHeader`.
- Keep messages lowercase, short, with location: `"empty input at 0"`.

## Good vs bad

Good (layered thiserror):

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("empty input at {0:?}")]
    EmptyInput(Span),
    #[error("invalid header: {0}")]
    InvalidHeader(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

Good (binary edge with context):

```rust
pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let text = std::fs::read_to_string(path).context("read config file")?;
    parse_config(&text).map_err(|parse_error| anyhow::anyhow!("parse config: {parse_error}"))
}
```

Bad (anyhow in library, unwrap in path):

```rust
pub fn parse_file_bad(path: &str) -> anyhow::Result<SolidityFile> {
    let text = std::fs::read_to_string(path).unwrap();
    Ok(parse_text(&text).unwrap())
}
```

Good (ForgeLS edge maps to diagnostic):

```rust
pub fn to_diagnostic(parse_error: &ParseError) -> Diagnostic {
    Diagnostic::error(parse_error.span, parse_error.to_string())
}
```

## Checklist

1. Crate errors are `thiserror`, binary errors use `anyhow` with context.
2. `?` flow, no `unwrap` in server paths.
3. Parser errors always map to diagnostics.
