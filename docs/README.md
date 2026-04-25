# Documentation Index

Use this index to choose the smallest document that answers your question.

## If You Want To Install Or Run Codex Desktop

- [README](../README.md) is the user-facing entry point for prerequisites,
  local app generation, native packages, updater service commands, and
  troubleshooting.

## If You Are Maintaining Packages Or Runtime Behavior

- [Package and Runtime Maintenance](maintainers/package-runtime-maintenance.md)
  is the maintainer reference for source files, generated artifacts, package
  payload, launcher behavior, updater state, privileged install boundaries,
  crate versioning, and validation selection.
- [Webview Server Evaluation](webview-server-evaluation.md) records the current
  local-server model, risks, and acceptance criteria for future webview-serving
  changes.

## If You Are An Agent Picking Up Work

- Read `AGENTS.md` first for always-loaded rules.
- Use [Package and Runtime Maintenance](maintainers/package-runtime-maintenance.md)
  for detailed package and updater context that should not live in `AGENTS.md`.
- Use [Agentic Maintenance Policy](policies/agentic-maintenance.md) when
  deciding what to persist in tracked docs, what to leave as generated output,
  and what to treat as local session evidence.
