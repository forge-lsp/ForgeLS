---
name: forgels-rust
description: Rust best practices for ForgeLS with Apollo references. Readable code, guard clauses first, no redundancy, ASCII comments only.
license: MIT
compatibility: opencode
metadata:
  audience: contributors
  workflow: rust
  opencode/slash: "true"
  opencode/autoinvoke: "false"
---

# forgels-rust

Focus: readable, fast, and consistent Rust for ForgeLS. All code and comments are objective English, to the point.

## When to use me

Use when writing, editing, or reviewing Rust in `crates/*`. Load me before any Rust change. `forgels-qa` uses me to review. Read the matching file under `references/` before touching that area. Do not guess when a reference exists.

## Core rules

- English only in code, comments, and docs. Objective and to the point. One idea per line.
- ASCII only. No emoji. No em dash. Use hyphen `-` or colon `:`.
- Guard clause first. Happy path last. Flat over nested.
- Borrow by default. Clone only for a real owned snapshot. Small `Copy` by value.
- Fallible code returns `Result`. No `unwrap` or `expect` in server paths. Map parser errors to `Diagnostic`.
- Names carry meaning: role nouns for variables, verb plus object for functions. No `data`, `thing`, `tmp`, `handle`, `process_data`.
- `cargo fmt --all` before finish. Keep `cargo clippy --workspace -- -D warnings` green.

## Edit tooling (STRICTLY FORBIDDEN otherwise)

- Edit files ONLY with the dedicated file tools (`edit`, `write`).
- STRICTLY FORBIDDEN: editing file content through any program or
  workaround, including `python3 -c`, `python` scripts, `sed`, `awk`,
  `perl`, `echo` redirection, or heredocs.
- Shell (`bash`) is for read-only inspection and project commands only:
  `cargo`, `git`, `ls`, `grep`, `wc`. Never for writing file content.
- Reason: programmatic edits corrupt text silently (escaping, unicode,
  off-by-one ranges) and bypass tool safety checks.

## References (read before you change that area)

| File | Use when |
| ---- | -------- |
| `references/borrowing-cloning-copy.md` | Picking `&T` vs `T`, `Copy` types, `Span` by value |
| `references/option-result-flow.md` | `Option`, `Result`, `let else`, `?`, early return |
| `references/lazy-allocation.md` | `unwrap_or_else`, `ok_or_else`, `inspect_err`, `map_err` |
| `references/iterators-loops.md` | `for` vs iterators, `sum` vs `fold`, no throwaway `collect` |
| `references/comments-docs.md` | `//` vs `///` vs `//!`, `SAFETY`, `TODO` as issues |
| `references/imports.md` | `use` order, workspace vs crate imports |
| `references/functions.md` | Split helpers, Rule of Three, no flag params |
| `references/clippy-lints.md` | Clippy gate, `expect` with reason, workspace lints |
| `references/performance.md` | Measure first, release bench, stack vs heap, iter pipelines |
| `references/errors.md` | `thiserror` in crates, `anyhow` in binary, `?` flow |
| `references/testing.md` | Sentence test names, one behavior, `rstest`, `insta` |
| `references/generics-dispatch.md` | `impl Trait` vs `dyn Trait`, object safety |
| `references/type-state.md` | Builders, connect before send, `PhantomData` states |
| `references/pointers.md` | `&T`, `Box`, `Rc`, `Arc`, `Cell`, `Mutex`, `OnceLock` |
| `references/naming.md` | Variable, function, and type names |
| `references/forgels-mapping.md` | ForgeLS crate split, `Span` duty, no cross-language import |

## ForgeLS musts

- Reuse `forgels-syntax`: `Lexer`, `Parser`, `AstNode`, `Span`, `ParseError`, `ParseResult`.
- Every public AST type carries `Span`.
- Language crates stay independent. Never add one `forgels-lang-*` to another.
- New shared code goes to `forgels-syntax`, `forgels-protocol`, `forgels-common`, or `forgels-core`.
- Binary `fls` stays thin: args, config, route by extension.

## Good vs bad (naming and flow)

Good (clear names, guard first):

```rust
pub fn parse_text(input: &str) -> ParseResult<SolidityFile> {
    let Some(first) = lex_text(input).first() else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    };
    // Build file node from tokens.
    SolidityFile::from_tokens(input, first.span)
}
```

Bad (vague names, nested, bool compare):

```rust
pub fn handle(d: String) -> ParseResult<SolidityFile> {
    // Initialize the result variable for the file
    if d.is_empty() == false {
        return parse_text(d.clone());
    } else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    }
}
```

## Checklist

1. Relevant `references/` file read and applied.
2. Guard clause first. No `unwrap`, no `== true`, no dead code.
3. Names state roles and actions. Comments are short ASCII why-notes.
4. Edited only with file tools. No programmatic edit of any kind.
5. `cargo check --workspace` passes. Clippy and fmt clean.

## Sources

- Apollo Rust Best Practices, `book/` chapters 1-9: https://github.com/apollographql/rust-best-practices/tree/main/book
- Rust API Guidelines, naming: https://rust-lang.github.io/api-guidelines/naming.html
- Rust Atomics and Locks (pointer background): https://marabos.nl/atomics/
