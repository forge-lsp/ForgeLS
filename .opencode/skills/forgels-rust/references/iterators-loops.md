# Iterators and Loops

Scope: `forgels-rust` skill. Choose `for` or iterator chains for token and symbol passes.

Source: Apollo `book/chapter_01.md` section 1.5, `book/chapter_03.md` section 3.4.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Use `for` when you need early exit (`break`, `continue`, `return`) or side effects such as IO and logging.
2. Use iterators when you transform collections without early exit: `filter`, `map`, `enumerate`, `windows`, `collect`.
3. Remember iterators are lazy. Nothing runs until a consumer like `collect`, `sum`, or `for_each`.
4. Never collect an intermediate `Vec` just to pass it to the next step. Pass `impl Iterator` instead.
5. Prefer `.iter()` over `.into_iter()` unless you must consume the collection. For `Copy` items, `.iter()` is enough.
6. Prefer `.sum()` over `.fold()` for sums. Add the base after the sum.
7. Put each chain step on its own line. Break long chains into named helpers.

## Naming

- Name items concretely: `token`, `symbol`, `span`.
- Name closures by transform: `is_valid`, `to_span`.
- Avoid `x`, `tmp`, `data` in chains longer than one step.

## Good vs bad

Good (iterator transform, no intermediate vec):

```rust
pub fn valid_spans(tokens: &[Token]) -> Vec<Span> {
    tokens
        .iter()
        .filter(|token| !token.text.is_empty())
        .map(|token| token.span)
        .collect()
}

pub fn total_len(spans: &[Span]) -> u32 {
    spans.iter().map(|span| span.len()).sum()
}
```

Bad (needless intermediate collection):

```rust
pub fn total_len_bad(spans: &[Span]) -> u32 {
    let lens: Vec<u32> = spans.iter().map(|x| x.len()).collect();
    lens.into_iter().fold(0, |a, b| a + b)
}
```

Good (`for` with early exit):

```rust
pub fn find_missing(tokens: &[Token]) -> Option<Span> {
    for token in tokens {
        if token.text.is_empty() {
            return Some(token.span);
        }
    }
    None
}
```

## Checklist

1. Early exit uses `for`, pure transform uses iterators.
2. No throwaway `collect`, no `fold` for sums.
3. One chain step per line, concrete item names.
