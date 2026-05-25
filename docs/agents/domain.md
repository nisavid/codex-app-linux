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
- `docs/maintainers/fork-sync-policy.md` defines upstream sync policy for this
  fork.
- `docs/maintainers/package-runtime-maintenance.md` covers package, launcher, updater, and generated-artifact maintenance.
- `docs/maintainers/threat-model.md` describes repository trust boundaries and threat paths.
- `docs/policies/agentic-maintenance.md` describes what belongs in tracked docs, agent policy, and local session evidence.
- `docs/usage/support-routing.md` explains whether behavior belongs with
  OpenAI, the Linux-port upstream, or this fork.

## Upstream Terminology

Use the specific term when introducing or disambiguating a surface. After the
context is clear, concise terms such as `upstream`, `DMG`, or `app bundle` are
fine.

- `Linux-port upstream`: `ilysenko/codex-desktop-linux`, the git remote named
  `upstream`, and sync work that imports that repository's Linux conversion
  changes.
- `Official OpenAI Codex DMG`: the OpenAI-distributed macOS app artifact used
  as app-generation input.
- `Official OpenAI app bundle`: the `Codex.app` bundle extracted from the DMG
  and patched for Linux.
- `OpenAI-hosted services`: account, rollout, entitlement, remote-control, and
  other service-side behavior outside this fork's local packaging path.

## Port Integration Terminology

Use `port integration` for configurable build-time modules that adapt official app
behavior or local runtime helpers to this Linux port. The implementation path,
config file, and environment variables are `port-integrations/`,
`port-integrations.json`, and `CODEX_PORT_INTEGRATIONS_*`; mention those exact names
only when documenting source paths or config APIs.

Do not call port integrations features of Linux. They are port-authored
integrations for user-facing Codex app surfaces, and this fork enables the
supported integration set by default as part of the complete local package.

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
