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

## Feature Status

| Surface | Status | Notes |
| --- | --- | --- |
| Standard Codex app UI | Working | Built from the upstream macOS DMG and patched to launch under Linux Electron. |
| Native Linux packages | Working | Builds `.deb`, `.rpm`, and pacman packages under the `codex-app` identity and install layout. |
| Local updater | Working | Native packages install `codex-app-updater`, adapted from upstream update-manager work to check DMGs and rebuild local packages. |
| Codex CLI preflight | Working | The launcher and updater find or install `@openai/codex` when host tools allow it. |
| Tray, warm start, and Linux keybinds | Working with desktop variance | Desktop-environment support can vary, especially around tray and window behavior. |
| Browser annotations | Working where upstream support is enabled | Uses the bundled browser resources shipped with the generated app. |
| Linux Computer Use | Packaged; UI controls opt-in | Uses upstream Linux Computer Use support with local packaging/manifest compatibility fixes; requires host accessibility/input support. |
| NixOS flake | Working with pinned DMG hash | The fixed-output hash can temporarily lag after OpenAI republishes the DMG. |
| OpenAI server-gated features | Gated by account and rollout | Installing this fork cannot bypass upstream feature flags or account policy. |

## About This Fork

This fork is a downstream maintenance fork of
[`ilysenko/codex-desktop-linux`](https://github.com/ilysenko/codex-desktop-linux).
Its main local differences are:

- the `codex-app` app/package identity and `codex-app-updater` updater
  identity;
- XDG/FHS-aligned package paths, including `/opt/codex-app` for the generated
  app and `/usr/lib/codex-app` for package-private support;
- package versions derived from the OpenAI DMG app bundle instead of local
  timestamp builds;
- extra package, updater, release, and supply-chain validation around the
  inherited Linux conversion workflow;
- local compatibility fixes for packaging upstream Linux Computer Use support
  without implying that account-side feature gates are bypassed.

The upstream owners and contributors did, and continue to do, much of the core
Linux adaptation work that makes this fork useful. This fork's job is to keep a
specific local package identity, install layout, updater policy, and maintenance
workflow coherent on top of that base.

For the maintainer inventory of intentional fork differences, see
[Fork Divergences](docs/maintainers/fork-divergences.md).

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

`scripts/install-deps.sh` supports Debian/Ubuntu, Fedora, openSUSE, and Arch
Linux hosts. On openSUSE it uses non-interactive `zypper` to install
`nodejs-default`, `npm-default`, `python3`, `p7zip-full`, `curl`, `unzip`, and
the `devel_basis` pattern.

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

For a fresh package build, start by removing the generated app tree, cached DMG,
and old package outputs:

```bash
make clean build-app package
```

That rebuilds `codex-app/` from the current upstream DMG source, then builds the
native package format for your host. To choose a package format explicitly,
replace `package` with `deb`, `rpm`, or `pacman`.

Package outputs land in `dist/`:

| Target | Output |
| --- | --- |
| Debian | `dist/codex-app_<upstream-version>_<arch>.deb` |
| RPM / Fedora / openSUSE | `dist/codex-app-<upstream-version>-1.<arch>.rpm` |
| Arch Linux | `dist/codex-app-<upstream-version>-1-<arch>.pkg.tar.zst` |

Architecture names follow the package format: Debian uses `amd64`, `arm64`, or
`armhf`; RPM uses `x86_64`, `aarch64`, or `armv7hl`; pacman uses `x86_64` or
`aarch64`.

The package version comes from the upstream Codex app bundle's
`CFBundleShortVersionString`. For example, `26.422.30944 (2080)` becomes
`26.422.30944`.

Native packages are named `codex-app`. They declare replacement metadata for
the older `codex-desktop` package name where the package format supports it.
The installed launcher is `/usr/bin/codex-app`, and the app lives under
`/opt/codex-app`.

Before publishing packages, run the release gate with a trusted upstream DMG
hash. Set `CODEX_RELEASE_GPG_KEY` to produce detached signatures, and set
`REQUIRE_RELEASE_SIGNATURE=1` when public releases must fail without them:

```bash
CODEX_DMG_SHA256=<reviewed-dmg-sha256> \
REQUIRE_RELEASE_SIGNATURE=1 \
CODEX_RELEASE_GPG_KEY=<key-id-or-email> \
make release-gate
```

For a local signed rehearsal where signatures are optional, omit
`REQUIRE_RELEASE_SIGNATURE=1` and keep `CODEX_RELEASE_GPG_KEY` set.

The gate verifies the DMG hash, scans the generated app for high-confidence
Electron security anti-patterns, validates package metadata, writes
`dist/SHA256SUMS`, and checks package identities. When
`CODEX_RELEASE_GPG_KEY` is set, it also writes
`dist/SHA256SUMS.asc`, exports `dist/release-signing-key.asc`, and verifies the
detached signature against that public key in a temporary keyring. Unsigned
rehearsal runs omit those signature artifacts unless `REQUIRE_RELEASE_SIGNATURE=1`
is set.

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

## Linux Computer Use

Linux Computer Use support is packaged from upstream's Rust MCP backend. The
backend can inspect apps through AT-SPI, capture screenshots through GNOME Shell
or XDG Desktop Portal paths, and synthesize input through `ydotool` when the
host is configured for it.

The plugin manifest gate is applied by default so the backend can register on
Linux. The in-app Computer Use UI controls are opt-in because they touch
upstream rollout-gated UI paths. Enable them for a build with:

```bash
CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1 make build-app
```

To keep the opt-in across updater rebuilds, write the persisted setting used by
the patcher:

```bash
settings_dir="${XDG_CONFIG_HOME:-$HOME/.config}/codex-app"
mkdir -p "$settings_dir"
python3 - "$settings_dir/settings.json" <<'PY'
import json
import os
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = {}
if path.exists():
    try:
        parsed = json.loads(path.read_text() or "{}")
    except json.JSONDecodeError:
        parsed = {}
    if isinstance(parsed, dict):
        data = parsed
data["codex-linux-computer-use-ui-enabled"] = True
tmp = path.with_name(path.name + ".tmp")
tmp.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
os.replace(tmp, path)
PY
```

This local opt-in only controls Linux UI patching in the generated app. It does
not bypass OpenAI account policy, server-side availability, or host accessibility
and input prerequisites.

## Updates

Native packages install `codex-app-updater`, a `systemd --user` service that
checks for newer upstream DMGs, rebuilds the matching Linux package locally, and
uses `pkexec` only for the final package install step.

Current updater crate version: `0.6.2`.

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
