# Codex App for Linux

<p align="center">
  <img src="assets/codex.png" alt="Codex app icon" width="96" height="96">
</p>

Run OpenAI Codex on Linux.

The official Codex app is published for macOS. This project adapts the
upstream `Codex.dmg` into a Linux Electron app, then gives you a few practical
ways to run it: directly from a checkout, through a native package, or through
the Nix flake.

> [!NOTE]
> This is an unofficial community project. It does not redistribute OpenAI
> software; it automates a local conversion from the upstream Codex DMG.

## Who This Is For

- **Linux users** who want the Codex app on their workstation.
- **Packagers** who want `.deb`, `.rpm`, or pacman artifacts built from a local
  app tree.
- **NixOS users** who want the flake path and Electron patching handled for
  them.
- **Maintainers and agents** who need the deeper packaging, updater, and policy
  references outside the README.

## Quick Start

```bash
git clone https://github.com/nisavid/codex-app-linux.git
cd codex-app-linux
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
| Debian | `dist/codex-app_<upstream-version>_amd64.deb` |
| RPM | `dist/codex-app-<upstream-version>-1.x86_64.rpm` |
| Arch Linux | `dist/codex-app-<upstream-version>-1-x86_64.pkg.tar.zst` |

The package version comes from the upstream Codex app bundle. For example,
`26.422.30944 (2080)` becomes `26.422.30944.2080`.

Native packages are named `codex-app`. They declare replacement metadata for
the older `codex-desktop` package name where the package format supports it.
The installed launcher is `/usr/bin/codex-app`, and the app lives under
`/opt/codex-app`.

Install the newest package in `dist/`:

```bash
make install
```

## NixOS

The flake handles dependencies and Electron patching:

```bash
nix run github:nisavid/codex-app-linux
```

This installs the generated app into `codex-app/` in the current directory. For
a development shell:

```bash
nix develop github:nisavid/codex-app-linux
```

If `nix run` reports a fixed-output `hash mismatch`, the upstream DMG was likely
republished after the pinned hash changed. A scheduled GitHub Actions job
refreshes that hash on `main` once every 24 hours. Retry after the bot has had
time to run; if it still fails, open an issue.

## Updates

Native packages install `codex-app-updater`, a `systemd --user` service that
checks for newer upstream DMGs, rebuilds the matching Linux package locally, and
uses `pkexec` only for the final package install step.

Useful service commands after installing a native package:

```bash
make service-enable
make service-status
codex-app-updater status --json
```

The packaged launcher also starts the user service on a best-effort basis when
you open the app.

## Troubleshooting

Start with the launcher log:

```bash
sed -n '1,160p' ~/.cache/codex-app/launcher.log
```

Common next steps:

- blank window or splash hang: check whether something else is serving port
  `5175`;
- Codex CLI warning: install `@openai/codex` globally or under `~/.local`;
- stale app tree: rebuild with `./install.sh --fresh`;
- updater service issue: inspect
  `~/.local/state/codex-app-updater/service.log`.

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
