# LSP 3.18 Reference

Scope: protocol subset for the ForgeLS beta server. Source: LSP 3.18 specification.

- Spec: https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/
- Version: 3.18. Changelog head in the spec reads 3.18.0 (06/04/2026).
- This file is a working subset, not a copy. It covers framing, lifecycle,
  sync, diagnostics, hover, completion, definition, and workspace folders.
- For the rest, read the spec link above.

## Base protocol framing

- Each message is headers plus a JSON-RPC 2.0 body.
- Headers are ASCII `name: value` lines ended by `\r\n`. Headers end with an
  empty line, so `\r\n\r\n` always precedes the body.
- Required header: `Content-Length` (body byte count).
- Optional header: `Content-Type`, default `application/vscode-jsonrpc; charset=utf-8`.
- Body encoding is UTF-8.

Example:

```text
Content-Length: 85\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"textDocument/hover","params":{...}}
```

## JSON-RPC shapes

- Request: `{jsonrpc, id: integer|string, method: string, params?: array|object}`.
  Every processed request must get a response.
- Response: `{jsonrpc, id, result?: any, error?: {code, message, data?}}`.
  `result` is required on success (`null` when empty) and must be absent on error.
- Notification: `{jsonrpc, method, params?}`. No `id`. No response ever.

Error codes:

- `-32700` ParseError, `-32600` InvalidRequest, `-32601` MethodNotFound,
  `-32602` InvalidParams, `-32603` InternalError (JSON-RPC range).
- `-32002` ServerNotInitialized: request before `initialize` succeeded.
- `-32001` and below down to `-32099` are reserved; `-32000` starts the
  implementation range.
- ForgeLS uses `-32601` for unknown methods today.

## Lifecycle

Order: `initialize` request, `initialize` result, `initialized` notification,
then normal traffic. Shutdown order: `shutdown` request, `shutdown` result
(`null`), `exit` notification, process ends.

- `initialize` is sent once, first. Params: `processId`, `clientInfo?`,
  `locale?`, `rootPath?` (deprecated), `rootUri` (deprecated, wins over
  `rootPath`), `initializationOptions?`, `capabilities`,
  `workspaceFolders?` (when the client supports them).
- Before `initialize` succeeds: requests get error `-32002`, notifications
  are dropped except `exit`. Clients send nothing else until the result
  arrives. Servers send nothing except `window/showMessage`,
  `window/logMessage`, `telemetry/event`, `window/showMessageRequest`.
- After `shutdown`: further requests should error InvalidRequest. Only
  `exit` is allowed next. Clients wait for the shutdown result before exit.
- `exit` ends the process: code `0` when shutdown came first, else `1`.
- ForgeLS beta gap: pre-initialize `-32002` and post-shutdown InvalidRequest
  are not enforced yet. Tracked as a follow-up item.

## Text sync

- `textDocumentSync` kinds: `0` None, `1` Full, `2` Incremental.
  ForgeLS advertises `1` (Full).
- `didOpen`: `{textDocument: {uri, languageId, version, text}}`. Open and
  close must balance: at most one open per document without a close between.
  Content is client managed after open; the server must not read the file.
- `didChange`: `{textDocument: {uri, version}, contentChanges: [{text}]}`
  for full sync. The client must sync before asking for hover, completion,
  or similar, so results match document state.
- `didClose`: `{textDocument: {uri}}`. The server drops ownership.
- `languageId` avoids guessing from the extension. Solidity and Noir are not
  in the spec table; ForgeLS uses `solidity` and `noir`.

## Core types

- `Position {line, character}`: zero based, cursor between characters.
  Character units follow the negotiated `PositionEncodingKind`.
- `PositionEncodingKind`: `utf-8`, `utf-16` (default, servers must support),
  `utf-32`. ForgeLS beta gap: UTF-16 conversion for non-ASCII is a
  follow-up item.
- `Range {start, end}`: end is exclusive.
- `Location {uri, range}`. Definition may also return `LocationLink[]`
  when the client sets `linkSupport`.
- `TextDocumentIdentifier {uri}`. URIs travel as strings.
- `Diagnostic`: `{range, severity?, code?, codeDescription?, source?,
  message, tags?, relatedInformation?, data?}`. Severity `1` Error,
  `2` Warning, `3` Information, `4` Hint. Always send severity so clients
  do not guess. 3.18 allows `MarkupContent` messages only when the client
  sets `markupMessageSupport`.
- `MarkupContent {kind: MarkupKind, value}` with `plaintext` or `markdown`.

## Language features (beta scope)

- Hover (`textDocument/hover`): params are text position params.
  Result `Hover {contents, range?}` or `null`. Hover position usually points
  just left of the hovered character; meaning is language specific.
- Completion (`textDocument/completion`): result list or `null`. Clients do
  filtering and sorting; servers may steer via `filterText`/`sortText`.
  `isIncomplete: true` opts out of client-side re-filter while typing.
  Expensive fields (`detail`, `documentation`) can wait for
  `completionItem/resolve`.
- Definition (`textDocument/definition`): result `Location | Location[] |
  LocationLink[] | null`.
- Publish diagnostics (`textDocument/publishDiagnostics`): server owned.
  The server recomputes on change and pushes; an empty array clears.
  Pushes replace, never merge. Single-file languages clear on close;
  project languages recompute on open.

## Workspace folders

- Announced via `workspaceFolders` client capability plus the
  `workspaceFolders` initialize param. `rootUri` predates them.
- Server capability: `workspace.workspaceFolders {supported?,
  changeNotifications?}`.
- `workspace/workspaceFolders` is a server to client request returning
  `WorkspaceFolder[] | null` (`{uri, name}`).
- `workspace/didChangeWorkspaceFolders`: `{event: {added: [], removed: []}}`.
- ForgeLS reads folders from initialize and tracks `added`/`removed`
  dynamically, evicting files whose root is gone.

## Beta mapping

Implemented now:

- Framing, initialize with 4 capabilities, shutdown/exit codes,
  `-32601` for unknown methods, multi-root folders, full-sync
  open/change/close, unknown files ignored silently.

Open follow-ups (each has a Plane item):

- Strict lifecycle errors: `-32002` before init, InvalidRequest after shutdown.
- UTF-16 position conversion for non-ASCII text.
- Percent-decoding of `file://` URIs.
