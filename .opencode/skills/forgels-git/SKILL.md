---
name: forgels-git
description: Conventional commit manager for ForgeLS. Groups related files and writes short to the point messages.
license: MIT
compatibility: opencode
metadata:
  audience: maintainers
  workflow: git
  opencode/slash: "true"
  opencode/autoinvoke: "false"
---

# forgels-git

Focus: commit management and history hygiene for ForgeLS.

## When to use me

Use when the user asks to commit, group changes, write a commit message, review staged files, or clean history. Ask before push, amend, or force actions.

## Rules

- Use Conventional Commits only: `type(scope): subject`
  - Allowed types: `feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `build`, `ci`
  - Scope is the crate or area, for example `forgels-syntax`, `forgels-core`, `solidity`, `noir`, `docs`, `opencode`
  - Subject is lowercase, short, to the point, no trailing period. Max 72 chars.
- Group by concern. One commit per logical change.
  - Do not mix feature with refactor, docs with code, or two languages in one commit.
  - Files in one commit must be relevant to the commit name. Split the rest.
  - Suggested grouping objectively: group by crate or area, so one commit holds one concern. Split docs, code, and separate languages into different commits.
- Description is optional and short. Use bullets. Explain what and why, not how.
- Never commit secrets, scratch temp notes, or build output.

## Workflow

1. Inspect: `git status --short`, `git diff --stat`, `git diff`, `git log --oneline -10`.
2. Propose groups: list each group with type, scope, subject, and file list. Wait for approval if more than one group.
3. Stage only intended files: `git add <paths>`. Never use `git add -A` on a dirty tree without review.
4. Commit per group: `git commit -m "type(scope): subject" -m "Body bullets if needed"`.
5. Verify: `git status --short`, `git log --oneline -3`, `git show --stat HEAD`.

## Examples

Good:

```bash
git add crates/forgels-syntax/src/span.rs crates/forgels-syntax/src/error.rs
git commit -m "feat(forgels-syntax): add span and parse error" -m "- Add Span with len check" -m "- Add ParseError with span context"
```

```bash
git add docs/architecture.md
git commit -m "docs(architecture): clarify crate dependency rules"
```

Bad:

- `fix: fix stuff and docs and refactor` (mixed concerns)
- `feat(solidity): update parser` with noir files inside (irrelevant files, objectively breaks one concern per commit)
- Long subject with trailing period and vague body.

## Safety

- Only commit, amend, push, or create PRs when explicitly requested.
- Never skip hooks, never force push, never commit empty changes unless asked.
- If hooks reject a commit, fix the issue and create a new commit. Do not amend a failed commit silently.
