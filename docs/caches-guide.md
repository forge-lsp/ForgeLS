# Caches Guide

Scope: `.caches/` folder. Local working area, not source of truth.

## Folders

- `plan/`: design notes and task plans.
- `eval/`: test reports and review results.
- `mem/`: durable memory such as decisions and open questions.
- `temp/`: scratch output. Gitignored. Safe to delete.

## Rules

- File names must be lowercase. Use hyphens.
- Notes use markdown with front matter: `title`, `date`, `status`, `owner`.
- Use mermaid charts for flows and architecture sketches.
- Promote final decisions to `docs/` in English.
