# Function Size and Extraction

Scope: `forgels-rust` skill. Small named helpers over long commented functions, without wrong abstractions.

Source: Apollo `book/chapter_01.md` sections 1.6 and 1.8, `book/chapter_08.md` section 8.5.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Split a long function when a comment must explain each step. The helper name replaces the comment.
2. Extract only when one of these holds: the logic is the same decision in 3 or more places, the name adds meaning the code lacks, it forms a unit worth testing, or it hides real complexity behind a small signature.
3. Do not extract 1-2 lines used once or twice. Duplication is cheaper than a wrong helper.
4. DRY means one representation per decision, not zero repeated lines. Coincidental look-alikes should stay separate.
5. Never add a `bool` flag parameter to merge two behaviors. Split into two functions or use a two-variant enum.
6. In tests, duplicate freely. Share setup and fixtures, keep each action and assertion inline.

## Naming

- Helper names are verbs with object: `validate_headers`, `resolve_symbol`, `span_from_token`.
- Never restate the body: `add_one_to_counter`, `do_parse_stuff`, `handle_data`.
- Flag-free names: `write_env_entry` and `write_toml_entry`, not `write_entry(quoted: bool)`.

## Good vs bad

Good (named steps, testable):

```rust
pub fn parse_request(text: &str) -> ParseResult<SolidityFile> {
    validate_header(text)?;
    let tokens = lex_text(text);
    build_file(&tokens)
}
```

Bad (commented block doing three jobs):

```rust
pub fn parse_request_bad(text: &str) -> ParseResult<SolidityFile> {
    // Validate header because empty files have no contract keyword
    // Then lex the text into tokens
    // Then build the file node from tokens
    unimplemented!()
}
```

Good (shared decision extracted):

```rust
pub fn is_retryable(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}
```

Bad (forced helper with flag):

```rust
pub fn write_entry_bad(out: &mut String, key: &str, value: &str, quoted: bool) {
    if quoted {
        out.push_str(&format!("{key} = \"{value}\""));
    } else {
        out.push_str(&format!("{key}={value}"));
    }
}
```

## Checklist

1. Third use triggers extraction, not the second.
2. No flag params, no single-use 2-line helpers.
3. Test bodies stay self contained.
