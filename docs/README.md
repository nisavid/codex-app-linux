# Documentation Index

Use this index to choose the smallest document that matches your goal.

## Run Or Install Codex

- [Project README](../README.md) is the landing page for users and potential
  users.
- [Build and Run Guide](usage/build-and-run.md) covers prerequisites, local app
  generation, native package builds, package installation, NixOS, and service
  commands.
- [Troubleshooting](usage/troubleshooting.md) lists common launch, CLI, webview,
  package, and updater symptoms.
- [User-Local App Integration](../contrib/user-local-install/README.md)
  describes the experimental rootless install layout under `~/.local`.

## Understand The Design

- [Webview Server Evaluation](webview-server-evaluation.md) explains why the
  launcher currently serves the extracted webview bundle with a local Python
  HTTP server and what would need to change before replacing it.
- [Changelog](../CHANGELOG.md) tracks user-visible releases and packaging
  behavior changes.
- [Contributors](../CONTRIBUTORS.md) records notable project contributions.

## Maintain Packaging Or Runtime Behavior

- [Package and Runtime Maintenance](maintainers/package-runtime-maintenance.md)
  is the reference for source files, generated artifacts, package payloads,
  launcher behavior, updater state, privileged install boundaries, versioning,
  and validation.
- [Agentic Maintenance Policy](policies/agentic-maintenance.md) explains what
  belongs in tracked docs, what belongs in agent policy, and what should remain
  local session evidence.

## Pick Up Agent Work

- Read [AGENTS.md](../AGENTS.md) first. It is the always-loaded policy surface.
- Use the package maintenance reference for details that are too large or too
  situational for `AGENTS.md`.
- Use repo-local skills under `.agents/skills/` when the task touches package
  metadata, launcher behavior, updater behavior, or generated install payloads.
