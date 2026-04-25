# AGENTS.md

## Repository Role

This repository adapts the official macOS Codex Desktop DMG into a Linux Electron app, builds native `.deb`, `.rpm`, and pacman packages, and ships the Rust `codex-update-manager` for local update checks, rebuilds, state tracking, and privileged package installation.

Treat this file as always-loaded agent policy. Keep detailed package recipes, runtime notes, and validation matrices in maintainer docs when those docs exist.

## Hard Rules

- Do not hand-edit generated app output as the durable fix. Change `install.sh`, package templates, updater code, or shared helpers, then regenerate or inspect generated output as needed.
- Treat `codex-app/`, `dist/`, `Codex.dmg`, and XDG updater config/state/cache paths as generated or runtime artifacts unless the task explicitly targets them.
- Do not assume `codex-app/` is pristine. If it disagrees with source scripts, source scripts win.
- Keep Linux package behavior in `packaging/linux/`, `scripts/build-deb.sh`, `scripts/build-rpm.sh`, `scripts/build-pacman.sh`, and `scripts/lib/package-common.sh`.
- Keep native-package-only launcher behavior in `packaging/linux/codex-packaged-runtime.sh`; `install.sh` should stay generic and load that helper only when packaging requires it.
- Keep package builders and `scripts/lib/package-common.sh` aligned when adding, removing, or moving packaged files.
- Preserve the unprivileged updater boundary. Escalation belongs only at install time through the updater's privileged install subcommands.
- If the updater crate version changes, update `updater/Cargo.toml`, user-facing version references, and maintainer versioning docs in the same change.

## Source Pointers

- Installer, ASAR patching, Electron runtime setup, generated launcher: `install.sh`
- Debian package builder: `scripts/build-deb.sh`
- RPM package builder: `scripts/build-rpm.sh`
- pacman package builder: `scripts/build-pacman.sh`
- Shared package staging helpers: `scripts/lib/package-common.sh`
- Host dependency bootstrap: `scripts/install-deps.sh`
- Linux package templates, maintainer scripts, desktop entry, service unit, packaged runtime helper: `packaging/linux/`
- Rust updater service and CLI: `updater/`
- User-facing overview and install guidance: `README.md`
- Webview server design decision and acceptance criteria: `docs/webview-server-evaluation.md`
- Future detailed maintainer notes, when present: prefer `docs/maintainers/` over expanding this file.

## Triggered Guidance

- Changing launcher behavior: edit `install.sh`; if package-only behavior is involved, edit `packaging/linux/codex-packaged-runtime.sh`; then regenerate or inspect `codex-app/start.sh`.
- Changing ASAR patches or Linux window behavior: edit the patching path from `install.sh` and `scripts/patch-linux-window-ui.js`; keep patches fail-soft when they target volatile upstream bundles.
- Changing webview serving: read `docs/webview-server-evaluation.md` before changing the local server model or port behavior.
- Changing package contents: update the relevant file under `packaging/linux/`, the affected package builder, and `scripts/lib/package-common.sh` together.
- Changing updater behavior: work in `updater/`, preserve persisted-state compatibility unless intentionally versioned, and check service/install behavior around failed, cancelled, or interrupted privileged installs.
- Changing update-manager service lifecycle: inspect `packaging/linux/codex-update-manager.service` and the package maintainer scripts for Debian, RPM, and pacman effects.
- Changing runtime CLI discovery or install behavior: keep the launcher best-effort; warnings may not block Electron startup unless the task explicitly changes that policy.
- Changing dependencies or supported runtime requirements: update `scripts/install-deps.sh`, `README.md`, and package metadata or maintainer docs as needed.

## Generated And Runtime Artifacts

- `codex-app/`: generated Linux app tree and launcher output.
- `dist/`: native package output.
- `Codex.dmg`: cached upstream DMG.
- `~/.config/codex-update-manager/config.toml`: updater runtime config.
- `~/.local/state/codex-update-manager/`: updater state and service logs.
- `~/.cache/codex-update-manager/`: downloaded DMGs, rebuild workspaces, staged packages, and build logs.
- `~/.cache/codex-desktop/launcher.log` and `~/.local/state/codex-desktop/app.pid`: launcher diagnostics and app liveness state.

Inspect generated artifacts to verify behavior, but do not make them the only source of a durable fix.

## Validation Policy

Choose the smallest validation set that covers the changed behavior.

- Shell changes: run `bash -n` on edited shell scripts.
- Updater changes: run `cargo check -p codex-update-manager` and targeted updater tests; run full updater tests for state, install, or CLI changes.
- Package changes: build the affected package format when practical and inspect package metadata plus the first package file listing.
- Launcher or installer changes: regenerate or inspect `codex-app/start.sh` and check launcher logs when runtime behavior is involved.
- Webview changes: verify the local server still serves expected Codex webview startup assets before Electron launch.

If a preferred validation cannot run because a host tool is missing, state the missing tool and run the closest useful static or targeted check.
