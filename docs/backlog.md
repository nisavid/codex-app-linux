# Backlog

This backlog tracks user-facing and maintainer follow-up that is not yet
covered by the focused security backlog. Keep it ordered by what should be
picked up next.

## Highest Priority

### Add a README feature-status table

The README should give end users a quick status view of major app features and
known gaps. Include working, partially working, gated, and unvalidated surfaces
such as standard Codex UI, native package formats, auto-update behavior, Linux
tray and warm-start behavior, browser annotations, Linux Computer Use, and
OpenAI server-gated features.

Desired state:

- the table is near the top of the README, before detailed setup steps;
- each row names the feature, current support level, and the most important
  caveat;
- Linux Computer Use is described as packaged and locally functional when host
  dependencies and OpenAI account rollout allow it;
- unvalidated desktop environments and host prerequisites are explicit without
  turning the README into maintainer notes.

### Add a README fork-divergence summary

The README should explain the practical differences between this fork and its
upstream so users understand what they are installing and what behavior is
intentional.

Desired state:

- the summary names this fork's intentional `codex-app` package/app identity and
  `codex-app-updater` service identity;
- upstream-derived names such as `codex-desktop` are mentioned only as package
  compatibility metadata where relevant;
- user-visible fork additions are grouped by theme, such as native Linux
  packaging, updater behavior, Linux launcher/runtime integration, and bundled
  Linux Computer Use support;
- maintainer workflow rules stay in `AGENTS.md` and maintainer docs rather than
  the README.

## Security Follow-Up

Security-specific follow-up is tracked in
[Security Backlog](maintainers/security-backlog.md).
