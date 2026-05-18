<div align="center">
  <img src="assets/codex.png" alt="Codex app icon" width="128" height="128">
  <h1>Codex App for Linux</h1>
  <p><strong>A polished local Codex desktop build for Linux package workflows.</strong></p>
  <p>
    <a href="#build-a-native-package"><img alt="Packages: deb, rpm, pacman" src="https://img.shields.io/badge/packages-deb%20%7C%20rpm%20%7C%20pacman-2f81f7?style=flat-square"></a>
    <a href="#local-updater"><img alt="Updater: codex-app-updater" src="https://img.shields.io/badge/updater-codex--app--updater-1f883d?style=flat-square"></a>
    <a href="#highlights"><img alt="Focus: hardening and polish" src="https://img.shields.io/badge/focus-hardening%20%2B%20polish-8250df?style=flat-square"></a>
  </p>
</div>

The official Codex app is published for macOS. This repository layers package
identity, updater policy, hardening, and runtime polish over the Linux
conversion work from
[`ilysenko/codex-desktop-linux`](https://github.com/ilysenko/codex-desktop-linux),
aimed at users who want a polished local app and maintainers who want auditable
native packages.

> [!NOTE]
> This is an unofficial community project. It does not redistribute OpenAI
> software; it automates a local conversion from the upstream Codex DMG.

## Highlights

- **Distro-shaped native packages.** Builds `.deb`, `.rpm`, and pacman packages
  under the `codex-app` identity, with `/opt/codex-app`, `/usr/lib/codex-app`,
  `/usr/bin`, `/usr/share`, and XDG user state arranged for package-managed
  installs. Package versions follow upstream app bundle metadata instead of
  local build timestamps.
- **Updater with a narrow privilege boundary.** `codex-app-updater` checks DMGs,
  rebuilds packages, tracks state, and recovers from failed or interrupted
  installs as an unprivileged service; only final package installation crosses
  through `pkexec`.
- **Managed runtime and CLI preflight.** Generated apps and native packages
  bundle the Linux Node.js runtime used by Browser Use, Codex CLI
  install/update, and updater rebuilds, reducing host dependency drift.
- **Release and supply-chain evidence.** The release gate verifies reviewed DMG
  hashes, scans generated Electron output, validates package metadata, writes
  checksums, supports detached signatures, and keeps upstream artifact trust
  explicit.
- **Computer Use packaging compatibility.** Upstream's Linux Computer Use
  backend is staged under this fork's package identity with manifest/path and
  input hardening, while local UI opt-in stays separate from OpenAI account and
  host accessibility gates.

## Feature Status

| Surface | Status | Notes |
| --- | --- | --- |
| Standard Codex app UI | Working | Built from the upstream macOS DMG and patched to launch under Linux Electron. |
| Native Linux packages | Working | Builds `.deb`, `.rpm`, and pacman packages under the `codex-app` identity and install layout. |
| AppImage self-build | Working | `make appimage` builds a local `dist/codex-app-*.AppImage`; AppImages are manual-update artifacts and do not bundle the updater service. |
| Local updater | Working | Native packages install `codex-app-updater`, adapted from upstream update-manager work to check DMGs and rebuild local packages. |
| Managed Node.js runtime | Working | Generated apps and native packages bundle the Node runtime used by Browser Use, CLI install/update, and updater rebuilds. |
| Codex CLI preflight | Working | The launcher and updater find or install `@openai/codex` when host tools allow it. |
| Tray, warm start, multi-instance, and Linux keybinds | Working with desktop variance | Normal launches reuse the running app; `--new-instance` or `CODEX_MULTI_LAUNCH=1` opens an isolated profile and bounded webview port. |
| Browser annotations | Working where upstream support is enabled | Uses the bundled browser resources shipped with the generated app. |
| Chrome plugin native host | Working | Stages the upstream Chrome plugin with Linux native-messaging support for Chrome, Brave, and Chromium. |
| Linux Computer Use | Packaged; UI controls opt-in | Uses upstream Linux Computer Use support with local packaging/manifest compatibility fixes; requires host accessibility/input support. |
| Remote control UI and mobile-control host patches | Opt-in experiment | Linux feature modules can expose upstream remote-control surfaces on Linux, but they do not bypass OpenAI account, rollout, MFA, or host-network requirements. |
| Linux feature registry | Working | `open-target-discovery` is enabled by default; feature config can disable it or enable other optional integrations before build. |
| NixOS flake | Working with pinned DMG metadata | The flake exposes default, Computer Use UI, remote-mobile-control, combined, and installer outputs. |
| OpenAI server-gated features | Gated by account and rollout | Installing this fork cannot bypass upstream feature flags or account policy. |

## About This Fork

This fork is a downstream maintenance fork of
[`ilysenko/codex-desktop-linux`](https://github.com/ilysenko/codex-desktop-linux).
Upstream does the core Linux app conversion and runtime enablement. The
Highlights above are the local finishing layer this repository is responsible
for: package identity/layout, updater policy, hardening, security evidence, and
compatibility polish.

The upstream owners and contributors did, and continue to do, the Linux
adaptation work that makes this fork useful. This fork's job is to keep a
specific local package identity, install layout, updater policy, hardening
posture, and maintenance workflow coherent on top of that base. For the full
inventory of fork-specific contracts, see
[`docs/maintainers/fork-divergences.md`](docs/maintainers/fork-divergences.md).

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

`scripts/install-deps.sh` supports Debian/Ubuntu-family, Fedora, openSUSE, and
Arch-family hosts. On openSUSE it uses non-interactive `zypper` to install
`nodejs-default`, `npm-default`, `python3`, `p7zip-full`, `curl`, `unzip`,
`coreutils`, `tar`, and the `devel_basis` pattern.

`make build-app` downloads or reuses `Codex.dmg`, extracts the app, patches the
macOS bundle for Linux, rebuilds native modules, downloads a Linux Electron
runtime, and writes `codex-app/start.sh`.

The generated app bundles a managed Linux Node.js runtime. You do not need a
distro `nodejs` or `npm` package for normal installs, Browser Use, Codex CLI
install/update, or local auto-update rebuilds. Existing `nvm`, asdf, Volta, or
system Node installs remain valid optional user tooling.

On hardened systems where `/tmp` is mounted `noexec`, set `TMPDIR` and
`XDG_CACHE_HOME` to user-owned executable locations before installing or
building. See [Troubleshooting](docs/usage/troubleshooting.md) for a compact
workaround.

On first launch, the app can install the Codex CLI if it is missing, using the
bundled managed runtime. If you already have an `npm` command on your shell
`PATH`, you can install the CLI yourself:

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

If Electron runtime or header downloads are slow or blocked, use
`ELECTRON_MIRROR` or `ELECTRON_HEADERS_URL`; the
[Build and Run Guide](docs/usage/build-and-run.md) has the exact knobs.

For a side-by-side test app with a distinct app id and webview port:

```bash
make build-dev-app
make run-dev-app
```

Normal launches reuse a running app through the warm-start handoff. To start an
additional isolated instance instead, pass `--new-instance` or set
`CODEX_MULTI_LAUNCH=1`; the launcher chooses the first free webview port in a
bounded range and uses per-port pid, socket, log, and Electron user-data paths.

```bash
./codex-app/start.sh --new-instance
CODEX_MULTI_LAUNCH=1 CODEX_MULTI_LAUNCH_PORT_RANGE=5175-5199 ./codex-app/start.sh
```

## Linux Features

Linux-side feature modules live in `linux-features/`. This fork enables
`open-target-discovery` by default so the Open menus can discover Linux
terminals, editors, and file managers from the current desktop session.

To disable default features or enable other optional integrations, copy
`linux-features/features.example.json` to the git-ignored
`linux-features/features.json`, edit the `enabled` and `disabled` lists, then
rebuild. Packaged installs can use
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/linux-features.json` for the same
override shape; checkout builds ignore that persistent user file and use
`linux-features/features.json` or `CODEX_LINUX_FEATURES_CONFIG` instead.
See [`linux-features/README.md`](linux-features/README.md) for the feature
contract.

The `remote-control-ui` and `remote-mobile-control` feature modules are
experimental opt-ins for upstream remote-control surfaces on Linux. Treat them
as UI/runtime integration patches, not as an account-policy bypass: OpenAI
rollouts, MFA state, connected-client state, and host network exposure still
come from upstream services and your local environment.

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

For a full host bootstrap that installs dependencies, regenerates the app from a
fresh upstream DMG, builds the matching native package, and installs the newest
artifact from `dist/`, run:

```bash
make bootstrap-native
```

If dependencies are already installed, `make install-native` skips the
dependency bootstrap and runs the fresh app build, package build, and install
flow.

For a fresh package build, start by removing the generated app tree, cached DMG,
and old package outputs:

```bash
make clean build-app package
```

That rebuilds `codex-app/` from the current upstream DMG source, then builds the
native package format for your host. To choose a package format explicitly,
replace `package` with `deb`, `rpm`, or `pacman`.

To build a package without installing `codex-app-updater`, its user service, or
its polkit/update-builder support files, disable the updater at package build
time:

```bash
PACKAGE_WITH_UPDATER=0 make package
```

No-updater packages also remove stale `codex-app-updater.service` enablement
when installed over a default package.

Package outputs land in `dist/`:

| Target | Output |
| --- | --- |
| Debian | `dist/codex-app_<upstream-version>_<arch>.deb` |
| RPM / Fedora / openSUSE | `dist/codex-app-<upstream-version>-1.<arch>.rpm` |
| Arch Linux | `dist/codex-app-<upstream-version>-1-<arch>.pkg.tar.zst` |
| AppImage | `dist/codex-app-<upstream-version>-<arch>.AppImage` |

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

Native packages bundle the managed Node.js runtime used by the launcher, Browser
Use, Codex CLI install/update flow, and local auto-update rebuilds. They do not
hard-depend on distro `nodejs` or `npm`.

For atomic desktops or systems where installing a native package is awkward,
build a local AppImage after `codex-app/` exists:

```bash
make appimage
./dist/codex-app-*.AppImage
```

The AppImage flow omits `codex-app-updater`, the systemd user service, polkit
policy, and the native-package update-builder bundle. Rebuild it manually when
you want a newer upstream Codex app.

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

The flake handles dependencies and Electron patching under the local
`codex-app` identity:

```bash
nix run github:nisavid/codex-app-linux
```

This installs the generated app into `codex-app/` in the current directory. For
a development shell:

```bash
nix develop github:nisavid/codex-app-linux
```

Feature-specific outputs are available when you want the generated app to carry
Linux feature opt-ins that are normally read from the git-ignored
`linux-features/features.json`:

```bash
nix run github:nisavid/codex-app-linux#codex-app-computer-use-ui
nix run github:nisavid/codex-app-linux#codex-app-remote-mobile-control
nix run github:nisavid/codex-app-linux#codex-app-computer-use-ui-remote-mobile-control
nix run github:nisavid/codex-app-linux#installer
```

If `nix run` reports a DMG metadata mismatch, the upstream DMG was likely
republished after the pinned metadata changed. A scheduled GitHub Actions job
refreshes that metadata and verifies the Nix package outputs on `main`. Retry
after the bot has had time to run; if it still fails, open an issue.

## Linux Computer Use

Linux Computer Use support is packaged from upstream's Rust MCP backend. The
backend can inspect apps through AT-SPI, capture screenshots through XDG Desktop
Portal or compositor paths, and synthesize input through `ydotool` when the host
is configured for it.

Runtime readiness depends on the host. Input synthesis usually requires
`ydotool`/`ydotoold`, `/dev/uinput` access, and a socket usable by your desktop
user. Non-GNOME desktops usually also need the matching XDG Desktop Portal
backend, such as the KDE or wlroots portal.

Keyboard input follows the desktop's active keyboard layout and remapping. When
troubleshooting literal keys or shortcuts on a non-QWERTY layout, retry once with
a standard US/QWERTY layout before debugging lower-level input services. Some
apps also expose only sparse AT-SPI trees even when the backend is ready;
screenshot and pointer paths can still work for those apps.

The plugin manifest gate is applied by default so the backend can register on
Linux. The in-app Computer Use UI controls are opt-in because they touch
upstream rollout-gated UI paths. Enable them for a build with:

```bash
CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1 make build-app
```

To keep the opt-in across updater rebuilds, write the persisted setting used by
the patcher:

If the existing file is missing, invalid JSON, or not a JSON object, this writes
a new JSON object containing only `"codex-linux-computer-use-ui-enabled": true`.

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

After building the app, the backend can report local readiness:

```bash
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux doctor
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux setup
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux apps
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux windows
```

## Local Updater

Native packages install `codex-app-updater`, a `systemd --user` service that
checks for newer upstream DMGs, rebuilds the matching Linux package locally, and
uses `pkexec` only for the final package install step.

Current updater crate version: `0.8.0`.

Useful service commands after installing a native package:

```bash
make service-enable
make service-status
codex-app-updater status --json
```

The packaged launcher also starts the user service on a best-effort basis when
you open the app.

If a rebuilt update installs but the previous retained package was better,
close Codex App and run:

```bash
codex-app-updater rollback
```

Rollback uses the last retained known-good package and refuses to run when no
rollback package is available.

## Troubleshooting

Start with the launcher log:

```bash
sed -n '1,160p' ~/.cache/codex-app/launcher.log
```

Common next steps:

- blank window or splash hang: check whether something else is serving port
  `5175`;
- Codex CLI warning: install `@openai/codex` globally or under `~/.local`;
- hardened `/tmp` with `noexec`: set `TMPDIR` and `XDG_CACHE_HOME` to
  executable user-owned paths before install/build;
- Electron download issues: retry, or set `ELECTRON_MIRROR` and
  `ELECTRON_HEADERS_URL` for your network;
- stale app tree: rebuild with `./install.sh --fresh`;
- Computer Use readiness: run the backend `doctor` command and check
  `ydotoold`, `/dev/uinput`, portal, and AT-SPI status;
- Fedora Computer Use input issue: some Fedora releases package the daemon as
  `ydotool.service` rather than `ydotoold.service`; if `doctor` reports
  `ydotool_socket: Permission denied`, confirm the socket is usable by users in
  the `input` group;
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
| Contribute a change | [Contributing](CONTRIBUTING.md) |
| Follow release notes | [Changelog](CHANGELOG.md) |
| Try the experimental rootless install path | [User-Local Desktop Integration](contrib/user-local-install/README.md) |
| Maintain packaging, launcher, or updater behavior | [Package and Runtime Maintenance](docs/maintainers/package-runtime-maintenance.md) |

For contributors and maintenance agents, start with `AGENTS.md`. It is the
always-loaded policy surface; detailed recipes and validation matrices live in
the docs linked above.
