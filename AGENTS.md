# AGENTS.md

## Repository Role

This repository adapts the official macOS Codex DMG into a Linux Electron app, builds native `.deb`, `.rpm`, and pacman packages, and ships the Rust `codex-app-updater` for local update checks, rebuilds, state tracking, and privileged package installation.

Treat this file as always-loaded agent policy. Keep detailed package recipes, runtime notes, and validation matrices in maintainer docs when those docs exist.

## Hard Rules

- `main` is protected. Before starting work, create and switch to a task branch.
- The first time a task branch is pushed, create a draft PR in the same workflow
  turn. Mark it ready only after local readiness gates pass and the PR body
  records verification evidence.
- Use `--repo nisavid/codex-app-linux` on every `gh pr` command in this
  checkout, including `create`, `view`, `ready`, `checks`, `merge`, and
  `status`. Do not rely on GitHub CLI's inferred repository; it can target the
  wrong repository in this fork checkout.
- Commit completed work before handoff. For long tasks, also commit at staged,
  functional cutoff points. Each commit must pass the normal checks for the
  changed surface before it is created.
- Before pushing changes that affect the generated app, installer, ASAR patcher,
  package builders, package payload, updater rebuild flow, or bundled runtime
  helpers, run a local app generation/build gate first. The minimum gate is a
  successful `./install.sh` or `make build-app` from the current sources plus the
  relevant local package builder when package contents are affected. Refresh
  `Codex.dmg` first unless the cached DMG was refreshed within the last 24
  hours. Record the exact DMG refresh or age-check command and build command in
  the verification notes.
- Use Conventional Commits. Commit messages must accurately describe the
  committed change.
- Do not hand-edit generated app output as the durable fix. Change `install.sh`, launcher templates, package templates, updater code, or shared helpers, then regenerate or inspect generated output as needed.
- Treat `codex-app/`, `codex-*-app/`, `dist/`, `Codex.dmg`, and XDG updater config/state/cache paths as generated or runtime artifacts unless the task explicitly targets them.
- Do not assume `codex-app/` is pristine. If it disagrees with source scripts, source scripts win.
- Keep Linux package behavior in `packaging/linux/`, `scripts/build-deb.sh`, `scripts/build-rpm.sh`, `scripts/build-pacman.sh`, and `scripts/lib/package-common.sh`.
- Preserve this fork's intentional names when syncing upstream: the app,
  install roots, launchers, package names, desktop files, and XDG app state use
  `codex-app`; the updater crate, binary, service, config, state, cache, and
  logs use `codex-app-updater`. Integrate upstream behavior under the local
  names instead of adopting upstream names.
- Preserve this fork's intentional layout when syncing upstream. Path decisions
  follow these criteria in order: the XDG Base Directory Specification, the
  Filesystem Hierarchy Standard, then common conventions used by mainstream
  Linux distros for modern Electron-style apps. Native packages keep the
  generated app bundle under `/opt/codex-app`, private package support under
  `/usr/lib/codex-app`, system launch and desktop integration under `/usr/bin`
  and `/usr/share`, and user runtime/config/cache/state under the appropriate
  XDG base directories. Do not adopt upstream `codex-app-linux` or
  `~/.local/opt` install roots as part of a sync.
- Preserve this fork's package version contract. Native package versions come
  from the OpenAI DMG app's `CFBundleShortVersionString`, written to
  `codex-app/codex-app-version.env` during app generation. Do not replace that
  with timestamp-based package versions during upstream syncs.
- When syncing upstream, use the user-global `syncing-forks-with-upstream`
  skill and the repo-local policy in `.agents/fork-sync-policy.toml`. Read
  `docs/maintainers/fork-divergences.md`,
  `.agents/fork-sync-policy.toml`, and
  `docs/maintainers/fork-sync-policy.md` before resolving conflicts. If the
  external skill is unavailable, follow the maintainer policy directly and
  record the missing-skill fallback in the sync ledger. Put uncertainty in the
  PR body for maintainer triage.
- Keep native-package-only launcher behavior in `packaging/linux/codex-packaged-runtime.sh`; `install.sh` should stay generic and load that helper only when packaging requires it.
- Keep package builders and `scripts/lib/package-common.sh` aligned when adding, removing, or moving packaged files.
- Preserve the unprivileged updater boundary. Escalation belongs only at install time through the updater's privileged install subcommands.
- If the updater crate version changes, update `updater/Cargo.toml`, `README.md`, `AGENTS.md`, and maintainer versioning docs in the same change.

## Source Pointers

- Installer, ASAR patching, Electron runtime setup, generated launcher: `install.sh`
- Launcher template and runtime behavior: `launcher/start.sh.template`
- Debian package builder: `scripts/build-deb.sh`
- RPM package builder: `scripts/build-rpm.sh`
- pacman package builder: `scripts/build-pacman.sh`
- Shared package staging helpers: `scripts/lib/package-common.sh`
- Linux Computer Use backend and bundled plugin: `computer-use-linux/` and `plugins/openai-bundled/plugins/computer-use/`
- Host dependency bootstrap: `scripts/install-deps.sh`
- Linux package templates, maintainer scripts, desktop entry, service unit, packaged runtime helper: `packaging/linux/`
- Rust updater service and CLI: `updater/`
- Updater crate version and versioning policy: `updater/Cargo.toml` and
  `docs/maintainers/package-runtime-maintenance.md` (current version: `0.6.2`)
- User-facing overview and install guidance: `README.md`
- Webview server design decision and acceptance criteria: `docs/webview-server-evaluation.md`
- Fork-specific contracts and upstream-sync review inventory: `docs/maintainers/fork-divergences.md`
- Upstream-sync policy, local gates, and sync ledger requirements:
  `docs/maintainers/fork-sync-policy.md` and `.agents/fork-sync-policy.toml`
- Security follow-up and `@codex-security` review routing: `docs/maintainers/security-backlog.md`
- Additional maintainer notes: prefer `docs/maintainers/` over expanding this file.

## Triggered Guidance

- Changing launcher behavior: edit `launcher/start.sh.template`; if install-time launcher identity or orchestration is involved, edit `install.sh`; if package-only behavior is involved, edit `packaging/linux/codex-packaged-runtime.sh`; then regenerate or inspect `codex-app/start.sh`.
- Changing ASAR patches or Linux window behavior: edit the patching path from `install.sh` and `scripts/patch-linux-window-ui.js`; keep patches fail-soft when they target volatile upstream bundles.
- Changing webview serving: read `docs/webview-server-evaluation.md` before changing the local server model or port behavior.
- Changing package contents: update the relevant file under `packaging/linux/`, the affected package builder, and `scripts/lib/package-common.sh` together.
- Changing updater behavior: work in `updater/`, preserve persisted-state compatibility unless intentionally versioned, and check service/install behavior around failed, cancelled, or interrupted privileged installs.
- Changing updater service lifecycle: inspect `packaging/linux/codex-app-updater.service` and the package maintainer scripts for Debian, RPM, and pacman effects.
- Changing runtime CLI discovery or install behavior: keep the launcher best-effort; warnings may not block Electron startup unless the task explicitly changes that policy.
- Changing dependencies or supported runtime requirements: update `scripts/install-deps.sh`, `README.md`, and package metadata or maintainer docs as needed.
- Syncing upstream: use the user-global `syncing-forks-with-upstream` skill,
  read `docs/maintainers/fork-divergences.md`,
  `.agents/fork-sync-policy.toml`, and
  `docs/maintainers/fork-sync-policy.md`, then triage incoming changes against
  the intentional fork contracts before pushing. If the external skill is
  unavailable, follow the maintainer policy directly and record that fallback in
  the sync ledger.

## Generated And Runtime Artifacts

- `codex-app/` and `codex-*-app/`: generated Linux app trees and launcher output.
- `codex-app/codex-app-version.env`: generated package-version metadata read
  from the upstream app bundle.
- `dist/`: native package output.
- `Codex.dmg`: cached upstream DMG.
- `~/.config/codex-app-updater/config.toml`: updater runtime config.
- `~/.local/state/codex-app-updater/`: updater state and service logs.
- `~/.cache/codex-app-updater/`: downloaded DMGs, rebuild workspaces, staged packages, and build logs.
- `~/.cache/codex-app/launcher.log` and `~/.local/state/codex-app/app.pid`: launcher diagnostics and app liveness state.

Inspect generated artifacts to verify behavior, but do not make them the only source of a durable fix.

## Validation Policy

Choose the smallest validation set that covers the changed behavior.

- Shell changes: run `bash -n` on edited shell scripts.
- Updater changes: run `cargo check -p codex-app-updater` and targeted updater tests; run full updater tests for state, install, or CLI changes.
- Package changes: build the affected package format when practical and inspect package metadata plus the first package file listing.
- Launcher or installer changes: regenerate or inspect `codex-app/start.sh` and check launcher logs when runtime behavior is involved.
- Webview changes: verify the local server still serves expected Codex webview startup assets before Electron launch.

If a preferred validation cannot run because a host tool is missing, state the missing tool and run the closest useful static or targeted check.
