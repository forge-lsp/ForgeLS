# ForgeLS Mapping

Scope: `forgels-rust` skill. How Apollo rules land in this repo.

Source: `AGENTS.md`, `docs/architecture.md`, `docs/syntax-toolkit.md`, plus Apollo `book/` habits.
Upstream book: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Shared logic lives in `forgels-syntax`, `forgels-protocol`, `forgels-common`, or `forgels-core`. Never in a language crate.
2. Language crates are independent. `forgels-lang-solidity` must never import `forgels-lang-noir`, and reverse. Check with `cargo tree -p forgels-lang-solidity`.
3. Reuse toolkit traits: `Lexer::tokenize`, `Parser::parse_text`, `AstNode::cast`, `Span`, `ParseError`, `ParseResult`.
4. Every public AST type carries a `Span`. No span-free node reaches diagnostics or goto-definition.
5. No `unwrap` or `expect` in server paths. Return `ParseResult` inside, map to `Diagnostic` at the edge.
6. New shared deps go to `[workspace.dependencies]` in root `Cargo.toml`, then `*.workspace = true` per crate. No per-crate pins.
7. Binary `forgels` stays thin: args, config, route by extension. Logic moves to `forgels-core` or the language crate.

## Naming in this repo

- Crates: `forgels-syntax`, `forgels-protocol`, `forgels-core`, `forgels-lang-solidity`, `forgels-lang-noir`.
- Language entry points: `LANGUAGE_ID`, `FILE_EXTENSIONS`, `lex_text`, `parse_text`.
- Analysis verbs: `collect_symbols`, `resolve_name`, `hover_at`, `complete_at`, `goto_definition`.
- Config nouns: `ServerConfig`, `default_config`, `config_path`.

## Good vs bad

Good (shared trait, span kept, error mapped):

```rust
use forgels_syntax::{Lexer, ParseResult, Span};

pub fn parse_contract(lexer: &impl Lexer, input: &str) -> ParseResult<ContractFile> {
    let tokens = lexer.tokenize(input);
    let Some(first) = tokens.first() else {
        return Err(ParseError::new(Span::new(0, 0), "empty input"));
    };
    ContractFile::from_tokens(&tokens, first.span)
}
```

Bad (language imports language, span dropped, unwrap):

```rust
use forgels_lang_noir::parser::parse_text;

pub fn parse_contract_bad(input: String) -> ContractFile {
    let file = parse_text(input.clone()).unwrap();
    ContractFile::new(file)
}
```

## Checklist

1. `cargo check --workspace` passes, no cross-language import.
2. Public nodes carry `Span`, errors map to `Diagnostic`.
3. `cargo fmt --all` run, clippy gate green.
