# Domain Docs

This is a single-context repo. Engineering skills should use one repo-wide domain context and one repo-wide ADR directory when those files exist.

## Before exploring, read these

- `AGENTS.md` for always-loaded repository policy.
- `docs/README.md` to choose the smallest maintained document for the task.
- `CONTEXT.md` at the repo root when it exists.
- `docs/adr/` when it exists, reading ADRs that touch the area about to change.

If `CONTEXT.md` or `docs/adr/` does not exist, proceed silently. Do not request those files before doing ordinary work.

## Current domain anchors

- `docs/maintainers/fork-divergences.md` is the canonical inventory of intentional fork differences.
- `docs/maintainers/fork-sync-policy.md` defines upstream-sync policy.
- `docs/maintainers/package-runtime-maintenance.md` covers package, launcher, updater, and generated-artifact maintenance.
- `docs/maintainers/threat-model.md` describes repository trust boundaries and threat paths.
- `docs/policies/agentic-maintenance.md` describes what belongs in tracked docs, agent policy, and local session evidence.

## File structure

```text
/
├── CONTEXT.md          (when present)
├── docs/adr/           (when present)
└── docs/
    ├── README.md
    ├── agents/
    ├── maintainers/
    ├── policies/
    └── usage/
```

## Use the glossary vocabulary

When `CONTEXT.md` defines a domain term, use that term in issue titles, plans, tests, and implementation notes. If the concept is missing from `CONTEXT.md`, prefer the vocabulary already used in `AGENTS.md` and the relevant maintainer doc.

## Flag ADR conflicts

If output contradicts an existing ADR, surface the conflict explicitly instead of silently overriding it.
