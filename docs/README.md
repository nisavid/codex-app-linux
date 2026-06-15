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
- [Support and Issue Routing](usage/support-routing.md) explains whether a
  behavior belongs with OpenAI, the Linux-port upstream, or this fork.
- [User-Local App Integration](../contrib/user-local-install/README.md)
  describes the experimental rootless install layout under XDG user paths.

## Understand The Design

- [Port Architecture](port-architecture.md) explains how the official OpenAI
  Codex DMG becomes a Linux Electron app and where replacement, patching, and
  launcher orchestration fit.
- [Port Integrations](../port-integrations/README.md) explains the configurable
  integration registry under `port-integrations/`.
- [Webview Server Evaluation](webview-server-evaluation.md) explains why the
  launcher currently serves the extracted webview bundle with a local Python
  HTTP server and what would need to change before replacing it.
- [Threat Model](maintainers/threat-model.md) describes trust boundaries,
  attacker assumptions, and priority threat paths for the Linux conversion,
  packaging, updater, and release flow.
- [Security Best Practices](maintainers/security-best-practices.md) turns the
  threat model into maintainer review rules for generated app patching and
  default-enabled port integrations.
- [Fork Divergences](maintainers/fork-divergences.md) is the canonical
  inventory of intentional differences from the Linux-port upstream, including
  names, paths, versioning, updater boundaries, Computer Use compatibility, and
  rename-aware sync checks.
- [Fork Sync Policy](maintainers/fork-sync-policy.md) defines the current
  upstream sync workflow, renamed-path reconciliation, sync ledger, local
  gates, and uncertainty triage.
- [Fork Sync Ledger](maintainers/fork-sync-ledger/) records durable summaries,
  special-handling notes, and follow-up decisions for broad upstream syncs.
- [Changelog](../CHANGELOG.md) tracks user-visible releases and packaging
  behavior changes.

## Maintain Packaging Or Runtime Behavior

- [Package and Runtime Maintenance](maintainers/package-runtime-maintenance.md)
  is the reference for source files, generated artifacts, package payloads,
  launcher behavior, updater state, privileged install boundaries, versioning,
  and validation.
- [README Visual Capture](maintainers/readme-visual-capture.md) defines the
  maintainer process for reproducible, non-sensitive README showcase assets.
- [Threat Model](maintainers/threat-model.md) is the repository-scoped security
  model for scans and reviews.
- [Security Best Practices](maintainers/security-best-practices.md) lists
  secure-by-default expectations for generated app patching, local helper
  bridges, desktop capture, and hosted-service gates.
- [Security Backlog](maintainers/security-backlog.md) points to security
  backlog issues and routes supply-chain review through `@codex-security`.
- [Remote Mobile Host Boundary Review](maintainers/remote-mobile-host-boundary-review.md)
  records the host-state matrix for remote-control and Codex mobile review.
- [Agentic Maintenance Policy](policies/agentic-maintenance.md) explains what
  belongs in tracked docs, what belongs in agent policy, and what should remain
  local session evidence.

## Pick Up Agent Work

- Read [AGENTS.md](../AGENTS.md) first. It is the always-loaded policy surface.
- [Backlog](backlog.md) points to open GitHub Issues for non-security and
  security follow-up.
- Use the package maintenance reference for details that are too large or too
  situational for `AGENTS.md`.
- Use repo-local skills under `.agents/skills/` when the task touches package
  metadata, launcher behavior, updater behavior, or generated install payloads.
