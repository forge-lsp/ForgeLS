# Clippy and Lint Discipline

Scope: `forgels-rust` skill. Lints as gates, not noise.

Source: Apollo full `book/chapter_02.md`.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Run this gate before finish:
```shell
cargo clippy --all-targets --all-features --locked -- -D warnings
```
2. Add the gate to CI. Never land red lints.
3. Respect at least: `redundant_clone`, `needless_borrow`, `map_unwrap_or`, `manual_ok_or`, `large_enum_variant`, `unnecessary_wraps`, `clone_on_copy`, `needless_collect`.
4. Fix the warning. Never silence with a bare `allow`.
5. If a lint is wrong for one spot, use `#[expect(...)]` with a reason comment. `expect` warns you when the lint stops firing.
6. Keep workspace lint config in root `Cargo.toml` under `[workspace.lints]`, then inherit per crate. Pin versions only at workspace level per ForgeLS rules.

## Naming

- Name the reason next to the `expect`: which tradeoff wins and why.
- Example: `// Faster match wins over size here; revisit past 2 KB payloads.`

## Good vs bad

Good (local expect with reason):

```rust
// Faster match wins over size for small fixed payloads.
#[expect(clippy::large_enum_variant)]
pub enum Message {
    Code(u8),
    Content([u8; 1024]),
}
```

Bad (global silence, no reason):

```rust
#[allow(clippy::all)]
pub fn parse_everything(text: &str) -> SolidityFile {
    unimplemented!()
}
```

Good (fix instead of silence):

```rust
pub fn token_text(token: &Token) -> &str {
    &token.text
}
```

Bad (needless borrow, clippy would flag):

```rust
pub fn token_text_bad(token: &Token) -> &str {
    &&token.text
}
```

## Checklist

1. Clippy gate is green with `-D warnings`.
2. No bare `allow`, every `expect` has a reason.
3. Workspace lint table owns the policy.
