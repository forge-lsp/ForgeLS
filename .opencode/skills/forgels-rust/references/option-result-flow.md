# Option and Result Flow

Scope: `forgels-rust` skill. Flat fallible flow for lexer, parser, and server handlers.

Source: Apollo `book/chapter_01.md` section 1.3, `book/chapter_04.md` sections 4.1-4.2 and 4.5.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Return `Result<T, E>` from fallible functions. Never panic for expected input errors.
2. Use `let ... else` for early return when the failure case needs no extra work.
3. Use `if let ... else` only when the failure branch needs its own computation.
4. Use `match` when you must inspect inner values or map one shape to another.
5. Propagate with `?`. No `unwrap` or `expect` in server paths. Tests may use them.
6. Never use `== true` or `== false`. Test the bool directly or with `!`.

## Naming

- Name results after the happy value: `file`, `span`, `symbol`.
- Name errors after the cause: `parse_error`, `missing_span`.
- Function names state fallibility plainly: `parse_text`, `resolve_name`, `load_config`.

## Good vs bad

Good (early return, flat):

```rust
pub fn first_token(tokens: &[Token]) -> ParseResult<&Token> {
    let Some(first) = tokens.first() else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    };
    Ok(first)
}

pub fn parse_or_default(text: &str) -> SolidityFile {
    if let Ok(file) = parse_text(text) {
        file
    } else {
        SolidityFile::empty()
    }
}
```

Bad (nested pyramid, bool compare):

```rust
pub fn first_token_bad(tokens: &[Token]) -> ParseResult<&Token> {
    if tokens.is_empty() == false {
        if tokens[0].text.is_empty() == false {
            return Ok(&tokens[0]);
        } else {
            return Err(ParseError::new(tokens[0].span, "empty token"));
        }
    } else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    }
}
```

Good (propagate with `?`):

```rust
pub fn load_and_parse(path: &str) -> ParseResult<SolidityFile> {
    let text = read_source(path)?;
    parse_text(&text)
}
```

Bad (unwrap in server path):

```rust
pub fn load_and_parse_bad(path: &str) -> SolidityFile {
    let text = read_source(path).unwrap();
    parse_text(&text).unwrap()
}
```

## Checklist

1. Guard clause first, happy path last.
2. `?` for propagation, `let else` for early exit.
3. No `unwrap`, no `expect`, no `== true` in server code.
