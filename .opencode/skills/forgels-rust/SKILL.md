---
name: forgels-rust
description: Rust best practices for ForgeLS. Readable code, guard clauses first, no redundancy, ASCII comments only.
license: MIT
compatibility: opencode
metadata:
  audience: contributors
  workflow: rust
  opencode/slash: "true"
  opencode/autoinvoke: "false"
---

# forgels-rust

Focus: best practices, readability, and consistent Rust for ForgeLS.

## When to use me

Use when writing, editing, or reviewing Rust in `crates/*`. Load me before any Rust change. `forgels-qa` uses me to review.

## Style rules

- Keep code consistent with surrounding module. Match naming, error shape, and layout.
- Comments must be to the point:
  - One line (`//`) for brief notes only. Max one clear idea per line.
  - Multi line (`///` or `//!`) only for relevant public API docs. Still to the point.
  - ASCII keyboard only. No unicode symbols, no emoji, no em dash. Use hyphen `-` or colon `:`.
- No redundant code:
  - Remove dead code, unused imports, unused vars, duplicate logic.
  - Do not wrap simple code in extra layers, extra clones, or extra allocations.
  - Prefer existing helpers in `forgels-syntax`, `forgels-common`, `forgels-protocol`.
- Conditions must eliminate the false case first (guard clause):
  - Return early on error or empty input. Keep the happy path last and flat.
  - Never use `== true` or `== false`. Use the bool directly or with `!`.
  - Prefer `let ... else` and `?` over nested `if let` pyramids.

## ForgeLS specifics

- Reuse traits from `forgels-syntax`: `Lexer`, `Parser`, `AstNode`, `Visitor`, `Span`, `ParseError`.
- Every public AST type must carry `Span`.
- Return `ParseResult<T>`. No `unwrap` or `expect` in server paths. Map errors to `forgels-protocol::Diagnostic`.
- Language crates stay independent. Never add `forgels-lang-*` to another `forgels-lang-*`.
- Run `cargo fmt --all` before finish. Keep `cargo clippy --workspace -- -D warnings` clean.

## Good vs bad

Good (guard clause first, flat):

```rust
pub fn parse(&self, tokens: &[Token]) -> ParseResult<SolidityFile> {
    let Some(first) = tokens.first() else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    };
    if first.text.is_empty() {
        return Err(ParseError::new(first.span, "empty token"));
    }
    // Build file node from tokens.
    Ok(SolidityFile::new(SyntaxNode::new(0, first.span)))
}
```

Bad (nested, redundant, verbose):

```rust
pub fn parse(&self, tokens: &[Token]) -> ParseResult<SolidityFile> {
    // Initialize the result variable for the file
    if tokens.len() > 0 {
        if tokens[0].text.is_empty() == false {
            // Create and return the file node
            return Ok(SolidityFile::new(SyntaxNode::new(0, tokens[0].span)));
        } else {
            return Err(ParseError::new(tokens[0].span, "empty token"));
        }
    } else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    }
}
```

## Checklist

1. Guard clause first. Happy path last.
2. No `unwrap`, no `== true`, no dead code.
3. Short ASCII comments only where needed.
4. `cargo check --workspace` passes.
