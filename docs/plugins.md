# Plugins

Scope: `fls` external WASM plugins. Guest lint and format via a frozen ABI.

## Config

- `lint.external` lists local WASM files: `{"lint":{"external":[{"path":"plugins/check.wasm"}]}}`.
- Paths are local only. URLs fail to load. Absolute paths load as-is;
  relative paths join cwd. Paths canonicalize at load and display
  strings use the canonical path.
- `lint.rules` may list guest rule IDs once a guest declares them.
  `warn` downgrades a guest error to warning, `err` upgrades a
  guest warning to error, `off` drops the diagnostic. One
  `resolve()` applies the same levels to guests and builtins.
- Unlisted guest IDs keep the guest-declared severity. A missing
  or unknown severity string defaults to warning.
- Guest levels live in `fls.json` only. TOML cannot set guest
  levels in V1: any guest ID in TOML fails as an unknown rule.
- TOML `lint.external` is rejected: use `fls.json` (JSON-only V1).
- Unknown rule IDs are exit 2 in CLI. LSP warns with defaults
  instead. Guest IDs must be non-empty with no
  whitespace and a slash group, for example `example/guest-rule`.
- Guests load once at startup. Restart the binary to pick up edits.
  No hot-reload in V1.

## Sandbox

- Fuel is 10M per call for lint and format.
- Memory cap is 64 MiB per store via `StoreLimits`.
- Epoch timeout is 2s per call via `epoch_interruption` plus a
  helper thread that increments the epoch after 2s.
- Zero host imports: modules with any import are rejected.
  WASI modules fail cleanly at load.
- Engine plus compiled modules are reused across files. One store
  per guest per call with fuel, epoch deadline, and memory limiter.
- Numbers were picked for CI hardware: tiny guests finish in ms.
  Fuel and wall guards are both armed per call; which fires first
  is guest-dependent.

## ABI

Guests export:

```text
memory
forgels_plugin_abi_version() -> i32   # major, must be 1
forgels_plugin_abi_minor() -> i32     # minor, optional, absent means 0
alloc(size: i32) -> i32
free(ptr: i32, size: i32)         # optional, absent means no-op
lint(ptr: i32, len: i32) -> i64       # packed ptr<<32|len
format(ptr: i32, len: i32) -> i64     # packed ptr<<32|len
forgels_plugin_rules() -> i64         # optional, same packing
```

- Compat against host 1.0:

  | Guest | Result |
  | ----- | ------ |
  | 2.0 | reject at load, exit 2 |
  | 1.1 | warn once, then load |
  | 1.0 | silent, load |
  | 1 with no minor export | silent, load (minor defaults to 0) |

- Major mismatch fails fast before `forgels_plugin_rules`.
  Message names both sides: `unsupported abi 2.0, want 1.x`.
- Minor ahead warns once per guest path per process at startup
  load. Message names both sides: `newer abi 1.1, want 1.0`.
  Repeat refs to the same canonical path warn only on first load.
- Exit mapping is unchanged: major reject is exit 2 in CLI, LSP
  warns with defaults instead. Minor warn still exits 0 or 1
  from diagnostics alone.

- Packing is frozen: `packed = (ptr << 32) | len` as `i64`.
- Host writes request JSON into guest memory via `alloc`, calls
  `lint` or `format`, reads output JSON, validates UTF-8, then frees
  both allocations via `free` when the guest exports it.
- `lint` and `format` exports are checked per file call, not at
  load. A missing export skips that guest for the file; builtins
  still report.
- Caps: request over `i32::MAX` and output over 16 MiB are
  rejected. Rejection skips that guest for the file; builtins
  still report.
- Request shapes:
  - lint: `{"path":"a.sol","text":"..."}`
  - format: `{"path":"a.sol","text":"...","line_length":120,"tab_width":4,"style":"space"}`
- Response shapes:
  - lint: `{"diagnostics":[{"rule":"example/guest-rule","message":"...","start":0,"end":1,"severity":"warning","fix":{"start":0,"end":1,"replacement":"x"}}]}`
  - format: `{"edits":[{"start":9,"end":10,"replacement":"B"}]}` or `{"text":"full replacement"}`
  - rules: `["example/guest-rule"]`
- Severity strings are `error`/`err` and `warning`/`warn`
  case-insensitive; unknown falls back to `warning`. Severity is
  per-diagnostic output; the `rules` array carries IDs only.
- Spans clamp to file length. Empty or whitespace rule IDs drop.
  Fixes become safe `LintFix`, edits become `FormatEdit`.

## Rust authoring

No in-repo example crate in V1. Shape for an author:

1. Create a `cdylib` or `bin` targeting `wasm32-unknown-unknown`
   with no WASI imports (no `std::fs`, no `std::net`, no clocks).
2. Export linear `memory` plus the functions above with C ABI.
   `free` is optional.
3. Implement a bump `alloc` over a static heap past your data
   segments; `free` is optional and may be a no-op.
4. Return static JSON for `lint` and `format` or parse the request
   JSON from `(ptr, len)` and build a JSON answer.
5. Declare rule IDs via `forgels_plugin_rules` as a JSON array.
   Keep IDs slashed and whitespace free. The array carries IDs
   only; severity travels per diagnostic in `lint` output.
   Guest IDs that collide with builtins are skipped silently.
6. Build to `.wasm`, list it in `fls.json`, set levels in
   `lint.rules`, restart `fls`. Undeclared guest IDs in
   `lint.rules` are `UnknownRule`: CLI exits 2, LSP warns with
   defaults.

## Behavior

- Lint enablement uses top-level `plugin`: absent means all, listed
  means only those, empty means none. Unknown names are exit 2
  in CLI; LSP warns with defaults instead.
- Check runs builtins plus imports plus filename, then guest lint,
  then the plugin group gate for builtins only, then one `resolve()`.
  Group gate wins over per-rule levels. Guest `off` drops via resolve.
  Empty hosts keep builtins byte-identical.
- Guests always run when listed in `lint.external`, even when the
  plugin list is empty or omits every builtin group. Excluded
  files skip guests.
- Per-file guest failures (bad JSON, trap, missing export) skip
  that guest for the file; builtins still report.
- Format gate checks `forge:format`: absent or listed runs builtin
  convergence plus one guest pass plus builtin again. Unlisted skips
  builtins and runs guest passes only. Each builtin phase keeps its
  cap-4 fixpoint.
- Fix (`check --write`) is out of scope for plugin gating: it applies
  safe fixes to whatever diagnostics survived the gate.
- Exit codes match builtins in CLI: a guest error after resolve
  exits 1, warning only exits 0, a truly unknown guest ID exits 2.
  LSP warns with defaults instead of exiting.
- LSP publish runs the same filter plus resolve, so guest levels
  match `check` one to one.
- Code actions stay direct with no plugin gate in V1 by design.
- CLI unknown plugin exits 2 like unknown rules. LSP warns and runs
  with defaults instead of exiting.
- No new core test harness in V1: coverage is unit tests for
  `PluginSet` plus CLI goldens for check, format-off, and guests.
