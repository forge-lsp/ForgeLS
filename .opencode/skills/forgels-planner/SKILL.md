---
name: forgels-planner
description: Plane-first planner for ForgeLS. Mandatory Plane check before and after execution, with 5W1H Indonesian work items.
license: MIT
compatibility: opencode
metadata:
  audience: contributors
  workflow: plan
  opencode/slash: "true"
  opencode/autoinvoke: "false"
---

# forgels-planner

Focus: every ForgeLS task is bound to a Plane work item. No execution without a clear status.

Note on language: this skill file is English. Work items written to Plane MUST be Indonesian.

## When to use me

Use me for every multi step task in `crates/*`, docs, or CI. Load me together with `forgels-rust` and `forge-deslop` before planning or executing.

## Hard rules (MANDATORY)

1. MUST use Plane BEFORE and AFTER execution.
2. BEFORE execution: check for a relevant work item. If none exists, create one first. Then MUST update its status to In Progress before touching code.
3. AFTER execution: MUST move its status (Done when acceptance is met, Cancelled with a reason when dropped).
4. Every new finding (bug, gap, follow-up) MUST become a new work item. Never leave it only in chat or code comments.
5. Every work item MUST have an objective, to the point title and description.
6. Descriptions in Plane MUST be Indonesian with 5W + 1H points.
7. Status update is MANDATORY both BEFORE and AFTER. No silent exceptions.

## Project and states

- Default project: ForgeLS, id `f0c749b3-b4b9-4adf-ae17-73658650636e`, identifier `FORGE`.
- Never guess state UUIDs. Use this map for the ForgeLS project:
  - Backlog: `903cfd4a-7d37-4eee-ab4a-73ebd4cf00eb`
  - Todo: `191c6f7a-857d-4a92-82ce-d994dc1e6012`
  - In Progress: `c9fb5904-e587-486c-91d8-5611e10c4dd6`
  - Done: `b8e6ae0e-420e-46f6-874f-ec092d41865c`
  - Cancelled: `fc8ce308-18a1-4b9d-ae59-bf66bceab3c6`
- When in doubt, re-read via `plane_state list` with the `project_id` above.

## Work item format (Indonesian content required)

Title:

- Pattern: `[area] short name`
- Valid areas: `transport`, `workspace`, `syntax`, `diagnostics`, `solidity`, `noir`, `config`, `tests`, `docs`, `release`, `epic`.
- Short name: lowercase, max 60 chars, no emoji, no em dash.

Description:

- MUST be Indonesian.
- Objective and to the point. One idea per line or bullet.
- MUST contain 5W + 1H (write the labels in Indonesian):
  - Apa (What): what is done.
  - Kenapa (Why): objective reason, one sentence.
  - Di mana (Where): target crate and files, for example `crates/forgels-lang-solidity/src/parser.rs`.
  - Kapan (When): sprint or cycle, for example `Sprint 1` or `Sprint 2`.
  - Siapa (Who): executor role, for example `executor` or `qa`.
  - Bagaimana (How): measurable acceptance, for example `cargo test -p forgels-lang-solidity passes`.
- Store as simple HTML with one `p` tag per point so it renders in Plane. Example `description_html` (keep this example in Indonesian):

```html
<div><p>Apa: tambah handler initialize dengan capabilities sync, hover, completion.</p><p>Kenapa: editor hanya memakai fitur yang diiklankan.</p><p>Di mana: crates/forgels-core/src/server.rs.</p><p>Kapan: Sprint 1.</p><p>Siapa: executor.</p><p>Bagaimana: response initialize memuat capabilities yang dideklarasikan dan tidak lebih.</p></div>
```

## Workflow BEFORE execution

1. Find relevant candidates:
  - Run `plane_workitem search` with area and topic keywords, then `plane_workitem list` with the ForgeLS `project_id` to confirm.
  - Read the top 1-3 candidates via `plane_workitem retrieve`. Match Apa, Di mana, and Bagaimana against the current task.
2. If a relevant item exists:
  - Reuse it. Do not duplicate.
  - Update its status to In Progress via `plane_workitem update` with `state` = In Progress id. This satisfies the BEFORE status update duty.
3. If none exists:
  - Create one via `plane_workitem create` with the ForgeLS `project_id`, a `name` matching `[area] ...`, an Indonesian 5W + 1H `description_html`, and an honest `priority` (`high` only when it blocks beta).
  - Then update its status to In Progress via `plane_workitem update`. Do not start code before this status is recorded.
4. Write a short plan: 1-2 line goal, numbered steps per crate, files to touch, verification.

## Workflow AFTER execution

1. Verify first: `cargo check --workspace`, `cargo test --workspace` (or the relevant scope), `cargo clippy --workspace -- -D warnings`, `cargo fmt --all -- --check`.
2. Move status via `plane_workitem update`:
  - Done when every Bagaimana point is met.
  - Cancelled when dropped, with the reason in a comment.
  - Keep In Progress only for continued work, and comment what is missing.
3. Add a comment via `plane_workitem_comment create` with: result, changed files, verification commands and results. The comment body may be English; the work item title and description stay Indonesian.
4. Scan for new findings. Each finding MUST become a new work item via `plane_workitem create` in Indonesian 5W + 1H format. Examples: panic on empty input, wrong span, stale docs, red CI.

## Good vs bad

Good (Indonesian title + full 5W1H):

```text
[transport] handler initialize dan capabilities
Apa: jawab initialize dengan capabilities server.
Kenapa: editor hanya memakai fitur yang diiklankan.
Di mana: crates/forgels-core/src/server.rs.
Kapan: Sprint 1.
Siapa: executor.
Bagaimana: response initialize berisi capabilities yang dideklarasikan dan tidak lebih.
```

Bad:

- `fix bug parser` (no area, no Di mana, no measurable Bagaimana).
- `Update docs` (no Kenapa, no acceptance).
- Long English prose with no clear points (violates the Indonesian description rule).

## Checklist before finish

1. A relevant work item exists and is In Progress BEFORE code changes.
2. Title matches `[area] ...`, description is Indonesian 5W + 1H.
3. Each new finding has its own work item.
4. AFTER status moved to Done or Cancelled, plus a result and verification comment.
5. No em dash in Plane text. Use hyphen `-` or colon `:`.
