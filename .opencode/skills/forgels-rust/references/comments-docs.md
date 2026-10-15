# Comments and Documentation

Scope: `forgels-rust` skill. Comments explain why. Docs explain API.

Source: Apollo `book/chapter_01.md` section 1.6, full `book/chapter_08.md`.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Use `//` only when code cannot show the reason: safety contract, perf quirk, platform workaround, ADR link, non-obvious gotcha.
2. Never restate the code. Delete `// increment i`, `// call parse`, `// return result`.
3. Keep `//` to one idea per line. ASCII only. No emoji. No em dash.
4. Name key comments: `// SAFETY:`, `// PERF:`, `// CONTEXT:`.
5. If a function needs a paragraph to explain steps, split it into named helpers instead.
6. Use `///` for every public item: what it does, params, return, errors, panics, one runnable example.
7. Use `//!` at the top of `lib.rs` and each `mod.rs`: crate or module purpose plus quick start.
8. Turn `TODO` into a tracked work item. If a marker must stay, use `// TODO(issue #N): concrete task`.
9. Treat stale comments as bugs. Fix or delete on sight.

## Naming

- Comment labels are uppercase tags: `SAFETY`, `PERF`, `CONTEXT`.
- Doc headers use standard sections: `# Examples`, `# Errors`, `# Panics`, `# Safety`.

## Good vs bad

Good (why, with contract):

```rust
// SAFETY: caller guarantees `span` lies inside `text`.
pub fn slice_at(text: &str, span: Span) -> &str {
    &text[span.start as usize..span.end as usize]
}
```

Bad (restates code):

```rust
pub fn slice_at_bad(text: &str, span: Span) -> &str {
    // Slice text by span and return it
    &text[span.start as usize..span.end as usize]
}
```

Good (doc with example and errors):

```rust
/// Parse Solidity source into a typed file node.
///
/// # Examples
///
/// ```
/// # use forgels_lang_solidity::parser::parse_text;
/// let file = parse_text("contract C {}");
/// assert!(file.is_ok());
/// ```
///
/// # Errors
///
/// Returns [`ParseError`] when the contract header is missing or malformed.
pub fn parse_text(input: &str) -> ParseResult<SolidityFile> {
    parse_source(input)
}
```

## Checklist

1. `//` states why, never what.
2. Public API has `///` with example plus Errors or Panics when relevant.
3. No vague `TODO`, no stale note left behind.
