# fls

Scope: `fls` crate. LSP server plus command line check for Solidity.

## Usage

```bash
fls lsp [--log-level LEVEL]
fls check [PATH...] [--write] [--json]
```

- `lsp` runs the LSP server on stdio with Solidity wired in.
- `check` lints Solidity files. No path scans cwd recursively for `*.sol`.
- Explicit check paths accept files or dirs. Output is sorted and deduped.
- `--write` applies safe fixes in place.
- `--json` emits the engine JSON report as-is.
- Only files with warn/err findings appear; clean files stay silent.

## Flags

- `lsp [--log-level LEVEL]`: level is trace, debug, info, warn, or error. Flag wins over config.
- `check [PATH...]`: files or dirs. Empty means cwd scan.
- `--write`: write safe fixes. Only safe applicability applies.
- `--json`: machine output with 0-based ranges, fix, fixed, summary.

## Exit codes

- `0`: clean or warnings only.
- `1`: any error diagnostic.
- `2`: operational failure (unreadable file, bad write, unknown rule).

## Config

- Loads `forgels.toml` next to cwd, like the server.
- Missing file means defaults.
- Broken TOML means defaults plus a stderr warning.
- Unknown rule IDs fail with exit 2. Bad levels fall back with warning.

## Limits

- Single-file analysis: dep-aware rules run without tracked deps.
- Import resolution uses cwd or the config root.
- V1 is Solidity only. Noir stays out to keep the binary small.
