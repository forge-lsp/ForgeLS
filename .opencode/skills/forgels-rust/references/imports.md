# Imports and Module Layout

Scope: `forgels-rust` skill. Predictable `use` order for fast review.

Source: Apollo `book/chapter_01.md` section 1.7.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Order groups exactly: `std`, then external crates, then workspace crates, then `super::`, then `crate::`.
2. Keep one blank line between groups. No blank line inside a group.
3. Prefer one item per line for long lists. Let `rustfmt` own the layout.
4. Suggested `rustfmt.toml` for this order:
```toml
reorder_imports = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```
5. Never use glob imports in library code except for tests and preludes. Name what you use.

## Naming

- Use full paths in `use` so the source crate is visible: `forgels_syntax::Span`, not a bare `Span` from an unclear prelude.
- Keep module names short nouns: `lexer`, `parser`, `ast`, `analysis`.

## Good vs bad

Good (grouped, explicit):

```rust
use std::collections::HashMap;

use serde::Deserialize;

use forgels_syntax::Span;

use super::error::ParseError;
use crate::ast::SolidityFile;
```

Bad (mixed, globbed):

```rust
use crate::ast::*;
use std::collections::HashMap;
use super::error::ParseError;
use forgels_syntax::Span;
use serde::Deserialize;
```

## Checklist

1. Groups in std, external, workspace, super, crate order.
2. No unexplained glob `use`.
3. `cargo fmt` passes without manual reorder.
