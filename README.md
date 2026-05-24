<div align="center">
  <img src="assets/codex.png" alt="Codex app icon" width="128" height="128">
  <h1>Codex App for Linux</h1>
  <p><strong>A polished local Codex desktop build for Linux package workflows.</strong></p>
  <p>
    <a href="#quick-start"><img alt="Packages: deb, rpm, pacman" src="https://img.shields.io/badge/packages-deb%20%7C%20rpm%20%7C%20pacman-2f81f7?style=flat-square"></a>
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
> software; it automates a local conversion from the official OpenAI Codex DMG.

## Start Here

- **Normal package-managed app:** use [Quick Start](#quick-start).
- **NixOS:** use [NixOS](#nixos).
- **Checkout, custom DMG, or side-by-side test app:** use
  [Manual and Custom Builds](#manual-and-custom-builds).
- **AppImage or package details:** use
  [Native Package Details](#native-package-details).
- **Computer Use, updater, release, or maintainer work:** use
  [Linux Computer Use](#linux-computer-use) and [Learn More](#learn-more).

## Quick Start

This is the normal fast path for a package-managed install. It removes old
generated output, rebuilds the Linux app from the official OpenAI Codex DMG,
builds the native package for your host, then installs that package with your
distro's package manager.

```bash
git clone https://github.com/nisavid/codex-app-linux.git
cd codex-app-linux
bash scripts/install-deps.sh
make clean build-app package
```

Install the package that `make package` wrote to `dist/`:

```bash
# Debian / Ubuntu
sudo apt install ./dist/codex-app_*.deb

# Fedora 41+
sudo dnf5 install ./dist/codex-app-*.rpm

# Fedora with dnf
sudo dnf install ./dist/codex-app-*.rpm

# openSUSE
sudo zypper --non-interactive --allow-unsigned-rpm install -y ./dist/codex-app-*.rpm

# Arch Linux
sudo pacman -U ./dist/codex-app-*.pkg.tar.zst
```

Then launch:

```bash
codex-app
```

`scripts/install-deps.sh` supports Debian/Ubuntu-family, Fedora, openSUSE, and
Arch-family hosts. The generated package bundles a managed Linux Node.js
runtime for normal app use, Browser Use, Codex CLI install/update, and updater
rebuilds.

On hardened systems where `/tmp` is mounted `noexec`, set `TMPDIR` and
`XDG_CACHE_HOME` to user-owned executable locations before installing or
building. See [Troubleshooting](docs/usage/troubleshooting.md) for a compact
workaround.

For an interactive preflight summary before building, run:

```bash
make setup-native
```

The guided setup helper detects the host package manager, desktop session,
package format, updater hints, Computer Use readiness signals, and optional
Linux feature config. It can write the git-ignored
`linux-features/features.json` file for the next build, but it does not run the
build, package, or install flow unless you explicitly opt in through
`CODEX_BOOTSTRAP_INSTALL_DEPS=1` or `CODEX_BOOTSTRAP_INSTALL_NATIVE=1`.

## Highlights

- **Distro-shaped native packages.** Builds `.deb`, `.rpm`, and pacman packages
  under the `codex-app` identity, with package-managed install roots and XDG
  user state. AppImage self-builds are available for manual-update systems.
- **Updater with a narrow privilege boundary.** `codex-app-updater` checks DMGs,
  rebuilds packages, tracks state, and uses `pkexec` only for final package
  installation.
- **Managed runtime and CLI preflight.** Native packages bundle the Linux
  Node.js runtime used by Browser Use, Codex CLI install/update, and updater
  rebuilds.
- **Release and supply-chain evidence.** The release gate verifies reviewed DMG
  hashes, scans generated Electron output, validates package metadata, writes
  checksums, and supports detached signatures.
- **Computer Use packaging compatibility.** The Linux-port upstream's Linux
  Computer Use backend is staged under this fork's package identity while UI
  opt-in, account rollout, and host accessibility gates stay separate.

## Current State

- **Working:** the standard Codex app UI, native packages, AppImage self-builds,
  local updater, managed runtime, Codex CLI preflight, Chrome native host,
  browser resources, and Linux feature registry.
- **Desktop-dependent:** tray behavior, warm start, multi-instance launches,
  and Linux keybind handling can vary by desktop environment.
- **Host-gated:** Linux Computer Use is packaged, but real readiness depends on
  local AT-SPI, screenshot portal or compositor support, `ydotool`, and input
  permissions.
- **Default Linux integrations:** remote-control UI, mobile-control host
  patches, Read Aloud, Read Aloud MCP, and conversation mode are enabled in the
  Linux feature registry by default. Account, rollout, MFA, connected-client,
  audio, and host-network requirements still apply.
- **NixOS:** the flake exposes the default app, Computer Use UI compatibility
  outputs, remote-mobile compatibility aliases, and installer outputs with
  pinned DMG metadata.
- **OpenAI-gated:** installing this fork cannot bypass server-side feature flags
  or account policy.

## About This Fork

This fork is a downstream maintenance fork of
[`ilysenko/codex-desktop-linux`](https://github.com/ilysenko/codex-desktop-linux).
The Linux-port upstream does the core Linux app conversion and runtime
enablement. This fork keeps the local `codex-app` package identity, install
layout, updater policy, hardening posture, and maintenance workflow coherent on
top of that base.

For the full inventory of fork-specific contracts, see
[`docs/maintainers/fork-divergences.md`](docs/maintainers/fork-divergences.md).

## Manual and Custom Builds

Use these paths when you do not want the normal package-managed install.

Build and run directly from the checkout:

```bash
make build-app
make run-app
```

`make build-app` downloads or reuses `Codex.dmg`, extracts the app, patches the
macOS bundle for Linux, rebuilds native modules, downloads a Linux Electron
runtime, and writes `codex-app/start.sh`.

On first launch, the app can install the Codex CLI if it is missing. To install
the CLI yourself with an existing `npm` command:

```bash
npm i -g @openai/codex
```

If global npm installs require elevated privileges on your system, use a
rootless prefix instead:

```bash
npm i -g --prefix ~/.local @openai/codex
```

Build from a DMG you already downloaded:

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

Linux-side feature modules live in `linux-features/`. This fork enables the
current Linux integration set by default: Open target discovery, remote-control
UI, mobile-control host patches, Read Aloud, Read Aloud MCP, and conversation
mode. Open target discovery lets the Open menus discover Linux terminals,
editors, and file managers from the current desktop session.

To disable default features or enable still-optional integrations, copy
`linux-features/features.example.json` to the git-ignored
`linux-features/features.json`, edit the `enabled` and `disabled` lists, then
rebuild. Packaged installs can use
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/linux-features.json` for the same
override shape; checkout builds ignore that persistent user file and use
`linux-features/features.json` or `CODEX_LINUX_FEATURES_CONFIG` instead.
See [`linux-features/README.md`](linux-features/README.md) for the feature
contract.

The `remote-control-ui`, `remote-mobile-control`, `read-aloud`,
`read-aloud-mcp`, and `conversation-mode` feature modules are default-enabled
Linux integration patches for the official OpenAI app bundle and local runtime
helpers.
Treat them as UI/runtime integrations, not as account-policy bypasses: OpenAI
rollouts, MFA state, connected-client state, audio availability, and host
network exposure still come from OpenAI-hosted services and your local
environment.

## Native Package Details

Native package builders repackage the generated app tree. The quick path uses
`make clean build-app package` so the app tree, cached DMG, and old package
outputs all start fresh.

If `codex-app/` already exists and you only need to rebuild the package, use:

```bash
make package
```

Choose a format directly when needed:

```bash
make deb
make rpm
make pacman
```

Convenience targets are available when you want Make to run more of the native
install lifecycle:

```bash
make bootstrap-native
make install-native
```

`make bootstrap-native` installs dependencies first, then runs the fresh app
build, package build, and install flow. `make install-native` assumes
dependencies are already present.

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
| Debian | `dist/codex-app_<app-version>_<arch>.deb` |
| RPM / Fedora / openSUSE | `dist/codex-app-<app-version>-1.<arch>.rpm` |
| Arch Linux | `dist/codex-app-<app-version>-1-<arch>.pkg.tar.zst` |
| AppImage | `dist/codex-app-<app-version>-<arch>.AppImage` |

Architecture names follow the package format: Debian uses `amd64`, `arm64`, or
`armhf`; RPM uses `x86_64`, `aarch64`, or `armv7hl`; pacman uses `x86_64` or
`aarch64`.

The package version comes from the official OpenAI app bundle's
`CFBundleShortVersionString`. For example, `26.422.30944 (2080)` becomes
`26.422.30944`.

Native packages are named `codex-app`. They declare replacement metadata for
the older `codex-desktop` package name where the package format supports it.
The installed launcher is `/usr/bin/codex-app`, and the app lives under
`/opt/codex-app`.

Native packages bundle the managed Node.js runtime used by the launcher, Browser
Use, Codex CLI install/update flow, and local auto-update rebuilds. They do not
hard-depend on distro `nodejs` or `npm`.

`make install` is a convenience wrapper around the package-manager install
commands shown in [Quick Start](#quick-start). It installs the newest matching
package in `dist/`.

For atomic desktops or systems where installing a native package is awkward,
build a local AppImage after `codex-app/` exists:

```bash
make appimage
./dist/codex-app-*.AppImage
```

The AppImage flow omits `codex-app-updater`, the systemd user service, polkit
policy, and the native-package update-builder bundle. Rebuild it manually when
you want a newer official OpenAI app bundle.

Before publishing packages, run the release gate with a trusted official OpenAI
Codex DMG hash:

```bash
CODEX_DMG_SHA256=<reviewed-dmg-sha256> \
REQUIRE_RELEASE_SIGNATURE=1 \
CODEX_RELEASE_GPG_KEY=<key-id-or-email> \
make release-gate
```

The release gate verifies the DMG hash, scans generated Electron output,
validates package metadata, writes checksums, and signs those checksums when
`CODEX_RELEASE_GPG_KEY` is set. `REQUIRE_RELEASE_SIGNATURE=1` makes the gate
fail without a signing key, which is the public-release mode; omit it for local
rehearsal runs. See the
[Build and Run Guide](docs/usage/build-and-run.md) and
[Package and Runtime Maintenance](docs/maintainers/package-runtime-maintenance.md)
for release details.

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
non-default compatibility options or legacy feature-specific aliases that would
otherwise be read from the git-ignored `linux-features/features.json`:

```bash
nix run github:nisavid/codex-app-linux#codex-app-computer-use-ui
nix run github:nisavid/codex-app-linux#codex-app-remote-mobile-control
nix run github:nisavid/codex-app-linux#codex-app-computer-use-ui-remote-mobile-control
nix run github:nisavid/codex-app-linux#installer
```

For a declarative NixOS or Home Manager install with the mobile remote-control
app-server managed by systemd, import the flake module:

```nix
{
  imports = [
    inputs.codex-app-linux.homeManagerModules.default
  ];

  programs.codexAppLinux = {
    enable = true;
    computerUseUi.enable = true;
    remoteMobileControl.enable = true;
    remoteControl.enable = true;
  };
}
```

`nixosModules.default` is also available for system-level configurations that
prefer a global user unit.

If `nix run` reports a DMG metadata mismatch, OpenAI likely republished the
Codex DMG after the pinned metadata changed. A scheduled GitHub Actions job
refreshes that metadata and verifies the Nix package outputs on `main`. Retry
after the bot has had time to run; if it still fails, open an issue.

## Linux Computer Use

Linux Computer Use support is packaged from the Linux-port upstream's Rust MCP
backend. The backend can inspect apps through AT-SPI, capture screenshots
through XDG Desktop Portal or compositor paths, and synthesize input through a
uinput absolute pointer, XDG Desktop Portal RemoteDesktop sessions, or
`ydotool` when the host is configured for them.

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
rollout-gated UI paths in the official OpenAI app bundle. Enable them for a
build with:

```bash
CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1 make build-app
```

This local opt-in only controls Linux UI patching in the generated app. It does
not bypass OpenAI account policy, server-side availability, or host accessibility
and input prerequisites. To keep the opt-in across updater rebuilds, set the
persisted `codex-linux-computer-use-ui-enabled` setting described in the
[Build and Run Guide](docs/usage/build-and-run.md).

After building the app, check backend readiness with:

```bash
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux doctor
```

## Local Updater

Native packages install `codex-app-updater`, a `systemd --user` service that
checks for newer official OpenAI Codex DMGs, rebuilds the matching Linux package
locally, and uses `pkexec` only for the final package install step.

Current updater crate version: `0.9.0`.

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
- stale app tree: rebuild with `make clean build-app package`, or use
  `./install.sh --fresh` for a checkout-only build;
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
| Build, run, package, install, or customize the app | [Build and Run Guide](docs/usage/build-and-run.md) |
| Diagnose launch, CLI, webview, or updater issues | [Troubleshooting](docs/usage/troubleshooting.md) |
| Set up or debug Linux Computer Use | [Build and Run Guide](docs/usage/build-and-run.md#linux-computer-use-ui-opt-in) and [Troubleshooting](docs/usage/troubleshooting.md) |
| Browse all repo docs by role and task | [Documentation Index](docs/README.md) |
| Contribute a change | [Contributing](CONTRIBUTING.md) |
| Follow release notes | [Changelog](CHANGELOG.md) |
| Try the experimental rootless install path | [User-Local Desktop Integration](contrib/user-local-install/README.md) |
| Maintain packaging, launcher, or updater behavior | [Package and Runtime Maintenance](docs/maintainers/package-runtime-maintenance.md) |

For contributors and maintenance agents, start with `AGENTS.md`. It is the
always-loaded policy surface; detailed recipes and validation matrices live in
the docs linked above.
