# Agentic Maintenance Policy

This repository is a small package-and-updater project. Leave enough durable
state in tracked files that a future maintainer or agent can continue without
chat history.

## What To Persist

Persist current desired behavior in the smallest durable place:

- `AGENTS.md` for always-loaded rules and high-value pointers;
- `README.md` for user-facing install, operation, troubleshooting, and version
  references;
- `docs/` for maintainer references, decisions, policies, and validation
  checklists;
- `packaging/linux/`, `scripts/`, `install.sh`, and `updater/` for executable
  package, launcher, and updater behavior.

When an investigation changes how the repo should be maintained, update a
tracked doc or source file before closing the work. Do not leave the only copy
of that knowledge in a chat transcript.

## What Not To Persist

Do not commit generated or machine-local runtime state as documentation.

Treat these as inspection evidence unless the task explicitly targets them:

- `codex-app/`
- `dist/`
- `Codex.dmg`
- XDG updater config, state, cache, and logs
- launcher logs and PID files
- temporary package build roots

Generated output can prove a source change worked, but the durable fix belongs
in the generator, package template, updater code, or maintained docs.

## Session Artifacts

Short-lived notes, command transcripts, and scratch outputs may help during a
session. Before closing substantial work, promote any lasting result into a
tracked file and leave the scratch artifact untracked or remove it if it no
longer has value.

Useful durable outcomes include:

- the current package payload contract;
- validation commands that passed or could not run;
- important runtime state-machine behavior;
- versioning or compatibility rules;
- reasons a generated artifact should not be edited directly.

Avoid durable docs that describe only what changed in one session. State the
current behavior directly.

## Fresh-Agent Handoff

A future agent should be able to start with:

1. `AGENTS.md`
2. [docs/README.md](../README.md)
3. [Package and Runtime Maintenance](../maintainers/package-runtime-maintenance.md)
4. the source file for the area being changed

If that path is insufficient after a task, update the docs so the next handoff
does not depend on hidden context.
