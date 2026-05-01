---
name: maintaining-codex-app-package
description: Use when changing native package metadata or payload, installer-generated launcher behavior, packaged runtime helper behavior, updater service or install behavior, or native package shape in the codex-app-linux repository.
---

# Maintaining Codex App Package

Use this skill for package and runtime maintenance in this repository.

Do not use it for README-only, policy-only, review-only, or generated-output-only turns unless the change also affects native package behavior, launcher generation, packaged runtime behavior, or updater install/service behavior.

## Start Discovery

Read these first:

1. `AGENTS.md`
2. `docs/README.md`
3. `docs/maintainers/package-runtime-maintenance.md`
4. `docs/maintainers/fork-divergences.md`
5. `docs/maintainers/fork-sync-policy.md`
6. `.agents/fork-sync-policy.toml`
7. Source files for the touched area

Then inspect the smallest relevant source set:

- Installer, ASAR patches, launcher template, and generated launcher:
  `install.sh`, `launcher/start.sh.template`, `scripts/patch-linux-window-ui.js`
- Native package builders: `scripts/build-deb.sh`, `scripts/build-rpm.sh`, `scripts/build-pacman.sh`
- Shared package staging: `scripts/lib/package-common.sh`
- Package templates, maintainer scripts, desktop entry, service unit, and
  packaged runtime helper: `packaging/linux/`, especially
  `packaging/linux/codex-packaged-runtime.sh`
- Linux Computer Use backend and bundled plugin: `computer-use-linux/` and
  `plugins/openai-bundled/plugins/computer-use/`
- Updater service and CLI: `updater/`

## Source Boundaries

Source scripts, templates, and updater code are the durable source of truth.
`codex-app/`, `codex-*-app/`, `dist/`, `Codex.dmg`, and XDG updater paths are
generated or runtime artifacts.

Inspect generated output to verify behavior, but do not make generated output the only fix.

When package contents move, keep the relevant package builder, `scripts/lib/package-common.sh`, and `packaging/linux/` files aligned.

During upstream syncs, preserve the fork contracts recorded in
`docs/maintainers/fork-divergences.md`,
`docs/maintainers/fork-sync-policy.md`, and `.agents/fork-sync-policy.toml`.
Use the global
`syncing-forks-with-upstream` skill before resolving conflicts, pushing, or
merging a broad upstream sync.

## Native Package Shape

For native package changes:

- Keep the `codex-app` package name, dependencies, replacement metadata, and installed paths aligned with the package contract.
- Keep compatibility metadata for older `codex-desktop` packages where the package manager supports it.
- Inspect generated package metadata with `dpkg-deb -I`, `rpm -qip`, or `pacman -Qip` when practical.
- Inspect package contents with `dpkg-deb -c`, `rpm -qlp`, or `pacman -Qlp` when practical.
- Keep payload paths consistent with launcher and updater expectations. The installed app paths are `/opt/codex-app`, `/usr/bin/codex-app`, `/usr/bin/codex-app-updater`, packaged runtime files, and the user service unit.

## Verification

Choose checks from `docs/maintainers/package-runtime-maintenance.md` that cover the changed behavior.

- Before pushing installer, generated-app, package, updater rebuild, or bundled
  runtime changes, refresh `Codex.dmg` or verify it was refreshed within the
  last 24 hours, then run `make build-app` or `./install.sh` from current
  sources.
- Shell changes: run `bash -n` on edited shell scripts.
- Updater changes: run targeted `cargo check` or updater tests.
- Package changes: build the affected package format when practical, then inspect metadata and the first package file listing.
- Launcher changes: regenerate or inspect `codex-app/start.sh`.

If a preferred tool is missing, record the missing tool and run the closest useful static check.

## Documentation

Update tracked docs when maintenance policy, package payload, installed paths, updater behavior, service lifecycle, package metadata, or the user-visible package contract changes.
