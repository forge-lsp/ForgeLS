# Slop Patterns Reference

Scope: `forge-deslop` skill. Objective reference for AI slop detection in ForgeLS.

This file lists common AI generated code traits that harm readability and review speed. Each entry states what it looks like, why it is forbidden, and how to fix it. Objective reason: less noise means faster review and fewer hidden bugs. Subjective reason: human code is direct and confident, it does not narrate itself or hedge with filler.

Use this file in review workflow: match `id` against changed lines, then apply `Fix` (remove, simplify, or flag).

## How to use

1. Scan changed files for each `id` below.
2. Quote the matched line in review output.
3. Apply fix: `remove-line`, `simplify`, or `flag` for human context.
4. Skip paths in `IGNORE_PATTERNS`.

## 1. Excessive comments

### verbose-obvious-comment (low, remove-line)

- What: comment that restates the code, for example `// Initialize the variable`, `// Call the function`, `// Return the result`.
- Why forbidden: it adds noise, hides real logic, and drifts from code on refactor. Readers can see the assignment or call already.
- Rust map: same rule for `//`. Keep only why or contract, for example `// Empty input is a caller error, fail fast.`
- Fix: delete the line. Keep code self evident.

### section-divider-comment (low, remove-line)

- What: decoration dividers such as `// --- Parse ---`, `// === Foo ===`.
- Why forbidden: it fakes structure, breaks `grep`, and creates merge noise. Use modules, functions, or headers in markdown instead.
- Rust map: same. Split code by function or module.
- Fix: delete the line.

### todo-placeholder (medium, flag)

- What: vague task such as `// TODO: fix this`, `// TODO: handle later`, `// TODO: implement functionality`.
- Why forbidden: it is not actionable, has no owner or scope, and rots in tree. It hides missing work from planner and QA.
- Rust map: same. Require context objectively: what task, where it blocks, plus an issue link or a follow up planning note.
- Fix: flag. Ask for clear task or delete if not needed.

## 2. Defensive programming overkill

### triple-null-check (medium, flag)

- What: triple guard such as `x != null && x != undefined && x != ""`.
- Why forbidden: it shows unclear null contract and duplicates checks. It makes happy path hard to read.
- Rust map: nested `is_none()` plus `is_some()` plus empty check on same value, or triple `match` on `Option` plus empty string. Prefer one clear contract: `Option` for absent, early return for empty, or `let ... else`.
- Fix: flag. Define one contract and use a guard clause.

### empty-catch-log (medium, flag)

- What: catch block that only logs, for example `catch (e) { console.error(e); }`.
- Why forbidden: it swallows the error, caller thinks it succeeded, and logs pile up without action.
- Rust map: `Err(e) => { eprintln!(e); }` with no propagate, or `catch_unwind` that only logs. Also `match` that logs and returns a default without reason.
- Fix: flag. Propagate with context (`?`, `map_err`), handle the case, or document why ignore with `let _ =`.

### unnecessary-try-catch (low, flag)

- What: try or catch around code that cannot throw, such as a plain assignment.
- Why forbidden: it suggests failure where none exists and adds nesting for no gain.
- Rust map: `Result` wrapper around infallible code, or `match` on a value that is never `Err`.
- Fix: flag. Remove the wrapper.

## 3. Verbose naming

### verbose-temp-variable (low, flag)

- What: filler temp names such as `temporaryVariableForX`, `valueToBeReturned`, `dataToProcess`.
- Why forbidden: long names add no meaning and force readers to track aliases. They often hide that a helper function is missing.
- Rust map: `let temporary_value_for_parsing`, `let result_of_the_lexer`. Prefer short clear names: `tokens`, `node`, `span`.
- Fix: rename. Extract helper if temp lives too long.

### response-data-chain (low, flag)

- What: deep chain such as `response.data.data.value`.
- Why forbidden: it leaks nested shape into many call sites. A shape change breaks all of them.
- Rust map: `resp.data.inner.value` repeated in many places, or `node.children[0].children[0]` chains.
- Fix: flag. Add accessor or destructure once at boundary.

## 4. Redundant code

### console-log-debug (high, remove-line)

- What: left over debug logs such as `console.log("DEBUG:...")`, `console.log("TEMP:...")`, `console.log("TODO:...")`.
- Why forbidden: it pollutes output, may leak data, and signals unfinished work.
- Rust map: `println!("DEBUG...")`, `dbg!(x)`, `eprintln!("TEMP...")` left in committed code.
- Fix: delete the line before QA.

### commented-console-log (low, remove-line)

- What: commented debug such as `// console.log(x)`.
- Why forbidden: dead code in comments rots fast and confuses readers. History already stores it.
- Rust map: `// println!(...)`, `// dbg!(...)`.
- Fix: delete the line.

### redundant-return (low, simplify)

- What: explicit empty return at end such as `return undefined; }`.
- Why forbidden: it is noise. The function already returns empty by falling off the end.
- Rust map: trailing `return ();` or `return x;` as last line where `x` alone is idiomatic.
- Fix: simplify to expression form.

### triple-equals-null (low, simplify)

- What: dual null check such as `x == null || x == undefined`.
- Why forbidden: two branches for one idea (absent). Pick one absent type.
- Rust map: `if opt.is_none() || s.is_empty()` used as one absent test in many places without a helper, or matching both `None` and `Some("")` separately each time.
- Fix: simplify. Use one helper such as `is_absent()` or one `Option` contract.

## 5. Over logging

### function-entry-log (medium, flag)

- What: log at start of every function such as `console.log("Entering foo")`.
- Why forbidden: it floods logs, adds no signal, and duplicates what a tracer or span already gives.
- Rust map: `println!("Entering parse")` or `log::info!("start")` at top of each function.
- Fix: remove. Use `Span` or tracing only where needed.

### function-exit-log (medium, remove-line)

- What: log at end such as `console.log("Exiting...")`, `console.log("Done")`.
- Why forbidden: same as entry log. Exit is visible from return value or caller.
- Rust map: `println!("Done")` before each return.
- Fix: delete the line.

## 6. Verbose coercion and compare

### string-coercion-verbose (low, flag)

- What: manual coerce such as `String(a) + String(b)`.
- Why forbidden: verbose and easy to get wrong for edge types. Template form is shorter and clearer.
- Rust map: `x.to_string() + &y.to_string()` where `format!("{x}{y}")` is clearer, or repeated `.to_string()` in a chain.
- Fix: flag. Use `format!` or `push_str` once.

### boolean-comparison (low, simplify)

- What: explicit bool compare such as `if (ok === true)`, `if (x == false)`.
- Why forbidden: redundant and error prone (`=` vs `==`). Bool already is a condition.
- Rust map: `if ok == true`, `if valid == false`, `assert!(x == true)`.
- Fix: simplify to `if ok`, `if !valid`.

## 7. Imports

### wildcard-import-inline (low, flag)

- What: star import plus heavy dotted use, for example `import * as u from ...` then `u.a`, `u.b` many times.
- Why forbidden: it hides what is used, hurts search, and risks name clash.
- Rust map: `use crate::foo::*;` plus many `foo::` style uses, or glob import that pulls unused names.
- Fix: flag. Import only used names explicitly.

## 8. Async

### await-promise-all-single (low, simplify)

- What: batch wrapper for one item such as `await Promise.all([x])`.
- Why forbidden: extra wrapper with no concurrency gain. It misleads readers to expect many tasks.
- Rust map: `join_all(vec![fut])` with one future, or `try_join!` with one branch, or spawning one task just to await it.
- Fix: simplify to direct `await`.

### async-no-await (medium, flag)

- What: async function with no await inside. Needs validation before flag.
- Why forbidden: false async contract. Callers pay async cost and expect yield where none happens.
- Rust map: `async fn` with no `.await`, or returning `impl Future` without need.
- Fix: flag. Make it sync or add missing await.

## Supported extensions

Apply checks to:

```text
.js, .jsx, .ts, .tsx, .mjs, .cjs,
.py,
.go,
.rs,
.java, .kt, .kts,
.c, .cpp, .cc, .cxx, .h, .hpp,
.cs,
.rb,
.php,
.swift,
.vue, .svelte
```

For ForgeLS, priority is `.rs` plus `.md` when docs or comments are touched.

## Ignore patterns

Skip these paths:

```text
node_modules, .git, dist, build, coverage,
.next, .nuxt, vendor, __pycache__, .venv, venv,
target, bin, obj
```

For ForgeLS, most relevant skip is the build output folder plus the scratch temp notes area.
