# Performance Mindset

Scope: `forgels-rust` skill. Measure first, optimize only with proof.

Source: Apollo full `book/chapter_03.md` sections 3.1 and 3.3-3.4.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Do not optimize without a measurement. Rule: no flamegraph or bench, no perf change.
2. Always bench with `--release`. Debug numbers prove nothing.
3. Use `cargo clippy -- -D clippy::perf` for cheap wins before hand tuning.
4. Write a micro bench when two designs compete. Keep an improvement only past about 5 percent.
5. Keep small `Copy` values on the stack. Pass huge values (over about 512 bytes) by reference.
6. Box recursive types. Avoid giant stack arrays. Do not add `#[inline]` without bench proof.
7. Pass iterators (`impl Iterator`) between stages instead of intermediate `Vec` buffers.

## Naming

- Bench names compare designs: `bench_lex_iter_vs_loop`.
- Name hot values by role: `token_window`, `symbol_table`.

## Good vs bad

Good (iterator pipeline, no buffer):

```rust
pub fn live_spans(tokens: &[Token]) -> impl Iterator<Item = Span> + '_ {
    tokens
        .iter()
        .filter(|token| !token.text.is_empty())
        .map(|token| token.span)
}
```

Bad (guess driven inline and buffer):

```rust
#[inline]
pub fn live_spans_bad(tokens: &[Token]) -> Vec<Span> {
    let kept: Vec<Span> = tokens.iter().map(|token| token.span).collect();
    kept
}
```

Good (recursive type boxed):

```rust
pub enum SyntaxTree {
    Leaf(Span),
    Branch(Box<SyntaxTree>, Box<SyntaxTree>),
}
```

## Checklist

1. Measurement exists before the change.
2. Release bench, clippy perf clean.
3. No giant by-value args, no unproven inline.
