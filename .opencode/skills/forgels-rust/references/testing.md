# Testing as Documentation

Scope: `forgels-rust` skill. Tests that read like specs for lexer, parser, and server sessions.

Source: Apollo full `book/chapter_05.md`, plus test part of `book/chapter_01.md` section 1.8.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Name tests as sentences: `returns_error_when_header_missing`, not `test_1` or `test_happy_path`.
2. Group by unit: `mod parse_text { ... }`. One behavior per test. One assertion per test when possible.
3. Share setup, not the test body. Fixtures and builders may be shared. Action plus assertion stays inline (DAMP over DRY in tests).
4. Use `rstest` cases with named cases for input tables. Keep case names descriptive.
5. Unit tests live with the unit and cover edge cases: empty input, huge input, invalid bytes, boundary spans.
6. Integration tests in `tests/` cover public API only: open file, edit, diagnostics, hover.
7. Doc tests show happy path usage and run under `cargo test`. Note: `cargo nextest` skips doc tests unless `cargo test --doc` also runs.
8. Prefer `assert_matches!` over `assert!(matches!(...))`. Always print the actual value on failure.
9. Use `insta` snapshots for structural output (AST dumps, diagnostics lists). Keep snapshots small, named, redacted for unstable fields, and committed to git.

## Naming

- Test modules mirror units: `mod span_len`, `mod parse_text`.
- Test fns state behavior: `rejects_empty_input`, `keeps_span_on_error`.
- Fixture names state shape: `empty_file`, `contract_with_two_fns`.

## Good vs bad

Good (one behavior, clear message):

```rust
#[cfg(test)]
mod parse_text {
    use super::*;

    #[test]
    fn rejects_empty_input() {
        let result = parse_text("");
        assert!(result.is_err(), "empty input must fail, got {result:?}");
    }

    #[test]
    fn keeps_span_on_bad_header() {
        let parse_error = parse_text("nope").unwrap_err();
        assert_eq!(parse_error.span, Span::new(0, 4));
    }
}
```

Bad (many behaviors, vague name):

```rust
#[test]
fn test_parser() {
    assert!(parse_text("").is_err());
    assert!(parse_text("contract C {}").is_ok());
    assert!(parse_text("nope").is_err());
}
```

Good (table with named cases):

```rust
#[rstest::rstest]
#[case::empty("")]
#[case::spaces("   ")]
fn rejects_blank_input(#[case] input: &str) {
    assert!(parse_text(input).is_err(), "input {input:?} must fail");
}
```

## Checklist

1. Sentence names, one behavior each.
2. Edge cases covered: empty, large, invalid, boundary.
3. Snapshots small, named, reviewed before accept.
