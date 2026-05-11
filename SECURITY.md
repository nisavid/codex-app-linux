# Security Policy

This repository is an unofficial community fork that adapts the official macOS
Codex app into Linux app and package formats. Security reports for this fork
should focus on the Linux conversion, package builders, generated launcher,
native packages, updater, bundled runtime helpers, local desktop integration,
and repository release workflows.

Security guarantees made by OpenAI services, OpenAI accounts, and the official
upstream Codex app outside this local conversion path are outside this
repository's scope.

## Supported Versions

Security work targets the current `main` branch and the latest package or
release artifacts published from this fork. Older package versions are not
maintained as separate security-support lines.

If you are using an older package build, update to the newest available package
or rebuild from current `main` before reporting an issue that may already be
fixed.

## Reporting A Vulnerability

Use GitHub's private vulnerability reporting flow for anything that may expose
users or package consumers. On the repository page, open **Security** and choose
**Report a vulnerability**.

Use private reporting for issues involving:

- updater downloads, rebuilds, state, cache, or privileged install boundaries;
- package builder inputs, package payloads, release checks, signing, or
  provenance;
- generated launcher behavior, local webview serving, desktop automation, or
  bundled runtime helpers;
- local file access, credentials, token handling, log redaction, or secret
  exposure;
- exploitable behavior in this fork's Linux packaging or conversion workflow.

Do not open a public issue for a suspected vulnerability before maintainers have
had a chance to triage it privately. Public issues are appropriate for ordinary
bugs, packaging failures, compatibility reports, documentation fixes, and
already-public hardening work.

## What To Include

Please include enough detail for maintainers to reproduce and scope the issue:

- affected commit, package version, or artifact name;
- Linux distribution, package format, and desktop environment when relevant;
- exact commands or user actions that trigger the behavior;
- relevant logs or command output with secrets removed;
- expected impact, affected trust boundary, and any known workaround.

## Maintainer Response

Maintainers triage private reports on a best-effort basis through GitHub
Security Advisories. When a report is valid, maintainers coordinate the fix,
local validation, and disclosure path before public details are published when
that is practical.

Depending on the issue, the outcome may include a private advisory, a patched
commit, updated package artifacts, release notes, maintainer documentation, or
a tracked public hardening task after sensitive details are no longer useful to
withhold.

For maintainer-facing security workflow and current open hardening work, see
[Security Backlog](docs/maintainers/security-backlog.md). For the repository
trust-boundary model, see [Threat Model](docs/maintainers/threat-model.md).
