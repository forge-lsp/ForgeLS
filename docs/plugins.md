# Plugins

Scope: `fls` external WASM plugins. Guest lint and format via a frozen ABI.

## Config

- `lint.external` lists local WASM files: `{"lint":{"external":[{"path":"plugins/check.wasm"}]}}`.
- Paths are local only. URLs fail to load. Relative paths join cwd.
- `lint.rules` may list guest rule IDs once a guest declares them.
- TOML `lint.external` is rejected: use `fls.json` (JSON-only V1).
- Unknown rule IDs stay exit 2. Guest IDs must be non-empty with no
  whitespace and a slash group, for example `example/guest-rule`.
- Guests load once at startup. Restart the binary to pick up edits.
  No hot-reload in V1.

## Sandbox

- Fuel is 10M per call for lint and format.
- Memory cap is 64 MiB per store via `StoreLimits`.
- Epoch timeout is 2s per call via `epoch_interruption` plus a
  helper thread that increments the epoch after 2s.
- Zero host imports: modules with imports are rejected, so fs, net,
  and clocks stay denied. WASI modules fail cleanly at load.
- Engine plus compiled modules are reused across files. One store
  per guest per call with fuel, epoch deadline, and memory limiter.
- Numbers were picked for CI hardware: tiny guests finish in ms,
  infinite loops trap on fuel well before the 2s wall.

## ABI

Guests export:

```text
memory
forgels_plugin_abi_version() -> i32   # must be 1
alloc(size: i32) -> i32
free(ptr: i32, size: i32)
lint(ptr: i32, len: i32) -> i64       # packed ptr<<32|len
format(ptr: i32, len: i32) -> i64     # packed ptr<<32|len
forgels_plugin_rules() -> i64         # optional, same packing
```

- Packing is frozen: `packed = (ptr << 32) | len` as `i64`.
- Host writes request JSON into guest memory via `alloc`, calls
  `lint` or `format`, reads output JSON, validates UTF-8, then frees
  both allocations via `free`.
- Request shapes:
  - lint: `{"path":"a.sol","text":"..."}`
  - format: `{"path":"a.sol","text":"...","line_length":120,"tab_width":4,"style":"space"}`
- Response shapes:
  - lint: `{"diagnostics":[{"rule":"example/guest-rule","message":"...","start":0,"end":1,"severity":"warning","fix":{"start":0,"end":1,"replacement":"x"}}]}`
  - format: `{"edits":[{"start":9,"end":10,"replacement":"B"}]}` or `{"text":"full replacement"}`
  - rules: `["example/guest-rule"]`
- Severity strings are `error`/`err` and `warning`/`warn`
  case-insensitive; unknown falls back to `warning`.
- Spans clamp to file length. Empty or whitespace rule IDs drop.
  Fixes become safe `LintFix`, edits become `FormatEdit`.

## Rust authoring

No in-repo example crate in V1. Shape for an author:

1. Create a `cdylib` or `bin` targeting `wasm32-unknown-unknown`
   with no WASI imports (no `std::fs`, no `std::net`, no clocks).
2. Export linear `memory` plus the functions above with C ABI.
3. Implement a bump `alloc` over a static heap past your data
   segments; `free` may be a no-op.
4. Return static JSON for `lint` and `format` or parse the request
   JSON from `(ptr, len)` and build a JSON answer.
5. Declare rule IDs via `forgels_plugin_rules` as a JSON array.
   Keep IDs slashed and whitespace free.
6. Build to `.wasm`, list it in `fls.json`, set levels in
   `lint.rules`, restart `fls`.

## Behavior

- Lint enablement uses top-level `plugin`: absent means all, listed
  means only those, empty means none. Unknown names fail fast.
- Check runs builtins plus imports plus filename, then guest lint,
  then the plugin group gate for builtins only, then one `resolve()`.
  Group gate wins over per-rule levels. Guest `off` drops via resolve.
  Empty hosts keep builtins byte-identical.
- Guests always run when listed in `lint.external`, even when the
  plugin list is empty or omits every builtin group.
- Format gate checks `forge:format`: absent or listed runs builtin
  convergence plus one guest pass plus builtin again. Unlisted skips
  builtins and runs guest passes only. Each builtin phase keeps its
  cap-4 fixpoint.
- Fix (`check --write`) is out of scope for plugin gating: it applies
  safe fixes to whatever diagnostics survived the gate.
- Code actions stay direct with no plugin gate in V1 by design.
- CLI unknown plugin exits 2 like unknown rules. LSP warns and runs
  with defaults instead of exiting.
- No new core test harness in V1: coverage is unit tests for
  `PluginSet` plus CLI goldens for check, format-off, and guests.
