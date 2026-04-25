# Codex Desktop for Linux

<p align="center">
  <img src="assets/codex.png" alt="Codex app icon" width="96" height="96">
</p>

Run OpenAI Codex Desktop on Linux.

The official Codex Desktop app is published for macOS. This project adapts the
upstream `Codex.dmg` into a Linux Electron app, then gives you a few practical
ways to run it: directly from a checkout, through a native package, or through
the Nix flake.

> [!NOTE]
> This is an unofficial community project. It does not redistribute OpenAI
> software; it automates a local conversion from the upstream Codex Desktop DMG.

## Who This Is For

- **Linux users** who want the Codex Desktop app on their workstation.
- **Packagers** who want `.deb`, `.rpm`, or pacman artifacts built from a local
  app tree.
- **NixOS users** who want the flake path and Electron patching handled for
  them.
- **Maintainers and agents** who need the deeper packaging, updater, and policy
  references outside the README.

## Quick Start

```bash
git clone https://github.com/nisavid/codex-desktop-linux.git
cd codex-desktop-linux
bash scripts/install-deps.sh
make build-app
make run-app
```

`make build-app` downloads or reuses `Codex.dmg`, extracts the app, patches the
macOS bundle for Linux, rebuilds native modules, downloads a Linux Electron
runtime, and writes `codex-app/start.sh`.

On first launch, the app can install the Codex CLI if it is missing and `npm` is
available. To install the CLI yourself:

```bash
npm i -g @openai/codex
```

If global npm installs require elevated privileges on your system, use a
rootless prefix instead:

```bash
npm i -g --prefix ~/.local @openai/codex
```

To build from a DMG you already downloaded:

```bash
make build-app DMG=/path/to/Codex.dmg
```

## Build A Native Package

Native package builders repackage the generated app tree. Run `make build-app`
first so `codex-app/` exists.

Build the package format that matches the current host:

```bash
make package
```

Or choose a format directly:

```bash
make deb
make rpm
make pacman
```

Package outputs land in `dist/`:

| Target | Output |
| --- | --- |
| Debian | `dist/codex-desktop_YYYY.MM.DD.HHMMSS_amd64.deb` |
| RPM | `dist/codex-desktop-YYYY.MM.DD.HHMMSS-<release>.x86_64.rpm` |
| Arch Linux | `dist/codex-app-YYYY.MM.DD.HHMMSS-1-x86_64.pkg.tar.zst` |

The Arch package is named `codex-app` and provides/conflicts with the older
`codex-desktop` package name. The installed launcher remains
`/usr/bin/codex-desktop`, and the app still lives under `/opt/codex-desktop`.

Install the newest package in `dist/`:

```bash
make install
```

## NixOS

The flake handles dependencies and Electron patching:

```bash
nix run github:nisavid/codex-desktop-linux
```

This installs the generated app into `codex-app/` in the current directory. For
a development shell:

```bash
nix develop github:nisavid/codex-desktop-linux
```

If `nix run` reports a fixed-output `hash mismatch`, the upstream DMG was likely
republished after the pinned hash changed. A scheduled GitHub Actions job
refreshes that hash on `main` once every 24 hours. Retry after the bot has had
time to run; if it still fails, open an issue.

## Updates

Native packages install `codex-update-manager`, a `systemd --user` service that
checks for newer upstream DMGs, rebuilds the matching Linux package locally, and
uses `pkexec` only for the final package install step.

Useful service commands after installing a native package:

```bash
make service-enable
make service-status
codex-update-manager status --json
```

The packaged launcher also starts the user service on a best-effort basis when
you open the app.

## Troubleshooting

Start with the launcher log:

```bash
sed -n '1,160p' ~/.cache/codex-desktop/launcher.log
```

Common next steps:

- blank window or splash hang: check whether something else is serving port
  `5175`;
- Codex CLI warning: install `@openai/codex` globally or under `~/.local`;
- stale app tree: rebuild with `./install.sh --fresh`;
- updater service issue: inspect
  `~/.local/state/codex-update-manager/service.log`.

See [Troubleshooting](docs/usage/troubleshooting.md) for the full symptom table
and log locations.

## Learn More

| Goal | Go here |
| --- | --- |
| Build, run, package, or install the app | [Build and Run Guide](docs/usage/build-and-run.md) |
| Diagnose launch, CLI, webview, or updater issues | [Troubleshooting](docs/usage/troubleshooting.md) |
| Browse all repo docs by role and task | [Documentation Index](docs/README.md) |
| Follow release notes | [Changelog](CHANGELOG.md) |
| Try the experimental rootless install path | [User-Local Desktop Integration](contrib/user-local-install/README.md) |
| Maintain packaging, launcher, or updater behavior | [Package and Runtime Maintenance](docs/maintainers/package-runtime-maintenance.md) |

For contributors and maintenance agents, start with `AGENTS.md`. It is the
always-loaded policy surface; detailed recipes and validation matrices live in
the docs linked above.
