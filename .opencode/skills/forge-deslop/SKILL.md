---
name: forge-deslop
description: Writing hygiene for ForgeLS. Blocks AI slop with objective reasons per pattern.
license: MIT
compatibility: opencode
metadata:
  audience: contributors
  workflow: review
  opencode/slash: "true"
  opencode/autoinvoke: "false"
---

# forge-deslop

Focus: clean writing in code, comments, docs, and notes. Avoid AI slop.

## When to use me

Use when writing or reviewing any text: Rust comments, official docs, agent rules, planning notes, commit messages. Load me together with `forgels-rust` for code and `forgels-git` for commits.

## Source of truth

Read the slop reference bundled in this skill folder before review. It explains each slop pattern objectively: what it looks like, why it harms readability, and how to fix it.

Read the reference and adapt JS oriented regex ideas to Rust and markdown:

- `verbose-obvious-comment`: remove comments that state the obvious (`// Initialize the variable`, `// Call the function`, `// Return the result`).
- `section-divider-comment`: remove `// --- Foo ---`, `// === Bar ===` decorations.
- `todo-placeholder`: flag vague `TODO: fix this` or `TODO: handle later`. Require context or issue link.
- `empty-catch-log`: flag catch that only logs. Handle or propagate with context.
- `console-log-debug`, `commented-console-log`, `function-entry-log`, `function-exit-log`: remove debug prints (`println!`, `dbg!`, `eprintln!`) and commented code. No entry or exit logs per function.
- `redundant-return`, `boolean-comparison`: simplify (`return x` without extra wrap, use `if ok` not `if ok == true`).
- `verbose-temp-variable`: rename `temporaryVariableForX` style to short clear names.
- `async-no-await`, `await-promise-all-single`: flag async without await or single item batch wrappers.

Also skip paths in `IGNORE_PATTERNS` (`target`, `build`, `dist`, `.git`, `node_modules`).

## Rules for ForgeLS

- Writing must be to the point. One idea per line or bullet.
- Plain natural ASCII only. No unicode tricks. No em dash. Use hyphen `-` or colon `:`.
- Official docs: English, direct, bullets and code over prose.
- Planning notes: lowercase filenames, markdown with front matter, mermaid for flows.
- Keep `SUPPORTED_EXTENSIONS` in mind: apply checks to `.rs` plus `.md`, `.js`, `.ts` when touched.

## Workflow

1. Read the slop reference bundled in this skill folder.
2. Scan changed files for the pattern ids there. Quote the matched line.
3. Fix by remove, simplify, or flag with context:
   - Remove: obvious comments, dividers, debug logs, commented code.
   - Simplify: bool compare, redundant return, verbose temp names.
   - Flag: vague TODO, empty catch, async without await. Ask for context.
4. Verify docs still ASCII only and with no em dash char at all.

## Examples

Remove:

```rust
// Initialize the result variable
let result = parse(text);
// --- Parsing section ---
println!("DEBUG: entering parse");
```

Keep:

```rust
// Empty input is a caller error, fail fast with span 0.
if text.is_empty() {
    return Err(ParseError::new(Span::new(0, 0), "empty input"));
}
```
