# Build and Run Guide

This guide is for users who want to run Codex on Linux or build a native
package from this repository.

## Prerequisites

You need:

- `python3`;
- `7z` or `7zz`;
- `curl`;
- `unzip`;
- `tar`;
- `make`;
- `g++` or equivalent C++ build tooling;
- Rust and `cargo` for `codex-app-updater`.

The installer downloads and bundles a managed Linux Node.js runtime for the
generated app, Browser Use, Codex CLI install/update flow, and updater rebuilds.
System `node`, `npm`, and `npx` remain useful for development and tests, but
normal app and package builds do not depend on distro Node.js packages.

The dependency helper supports `apt`, `dnf5`, `dnf`, `zypper`, and `pacman`:

```bash
bash scripts/install-deps.sh
```

On hardened systems where `/tmp` is mounted `noexec`, the Rust installer and
managed Linux Node.js runtime may fail when they try to execute temporary files.
Use executable user-owned locations for temporary and cache files before
running install or build commands:

```bash
mkdir -p ~/tmp/codex-work ~/tmp/codex-cache
export TMPDIR=~/tmp/codex-work
export XDG_CACHE_HOME=~/tmp/codex-cache
```

The generated launcher can install `@openai/codex` on first run when the CLI is
missing. To install it before launching:

```bash
npm i -g @openai/codex
```

If global npm installs require elevated privileges, install under `~/.local`:

```bash
npm i -g --prefix ~/.local @openai/codex
```

## Distro Notes

### Ubuntu And Pop!_OS

Ubuntu-family `p7zip-full` packages can be too old to extract newer APFS DMGs.
`scripts/install-deps.sh` bootstraps a newer `7zz` into `~/.local/bin` by
default. Set `SEVENZIP_SYSTEM_INSTALL=1` to install it under `/usr/local/bin`
instead.

To install `7zz` manually, download the current Linux tarball from
<https://www.7-zip.org/download.html>, then replace `<VERSION>` with the
published version:

```bash
curl -L -o /tmp/7z.tar.xz "https://www.7-zip.org/a/7z<VERSION>-linux-x64.tar.xz"
tar -C /tmp -xf /tmp/7z.tar.xz 7zz
install -d -m 755 "$HOME/.local/bin"
install -m 755 /tmp/7zz "$HOME/.local/bin/7zz"
```

### Fedora

Run the dependency helper:

```bash
bash scripts/install-deps.sh
```

It installs Python, 7z, curl, build tools, and bootstraps Rust through `rustup`
if `cargo` is missing. Fedora 41+ uses the app's managed Node.js runtime
instead of requiring distro `nodejs` and `npm` packages.

### Arch Linux

Run the dependency helper:

```bash
bash scripts/install-deps.sh
```

Or install the system packages directly:

```bash
sudo pacman -S --needed python p7zip curl unzip zstd base-devel
```

Install Rust through `rustup` if `cargo` is still missing:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### NixOS

Run the flake:

```bash
nix run github:nisavid/codex-app-linux
```

Or enter a development shell:

```bash
nix develop github:nisavid/codex-app-linux
```

The flake pins the SRI hash of the official OpenAI `Codex.dmg`. OpenAI
republishes the DMG at the same URL for each release, so the hash can
temporarily lag. A GitHub Actions job refreshes the hash on `main` once every
24 hours. If you see:

```text
error: hash mismatch in fixed-output derivation
```

retry after the scheduled job has had time to run. If the mismatch remains,
open an issue.

## Generate The Local App

```bash
make build-app
```

This creates `codex-app/` and writes the Linux launcher to
`codex-app/start.sh`.

Run the generated app:

```bash
make run-app
```

Equivalent direct command:

```bash
./codex-app/start.sh
```

If you want a shell shortcut for checkout builds:

```bash
echo 'alias codex-app="~/codex-app-linux/codex-app/start.sh"' >> ~/.bashrc
```

To use a DMG you already have:

```bash
make build-app DMG=/path/to/Codex.dmg
```

If Electron runtime or header downloads from the default endpoints are slow or
blocked, point the build at a mirror:

```bash
ELECTRON_MIRROR=https://npmmirror.com/mirrors/electron/ \
make build-app
```

`ELECTRON_HEADERS_URL` controls the Electron header URL passed to
`@electron/rebuild --dist-url`; it must provide both
`node-v<version>-headers.tar.gz` and the matching `SHASUMS256.txt`.

For a side-by-side test build with a distinct app id and webview port:

```bash
make build-dev-app
make run-dev-app
```

Override the side-by-side identity with Make variables:

```bash
DEV_APP_ID=codex-test DEV_APP_NAME="Codex Test" make build-dev-app
```

Override the webview port by exporting it for the build command:

```bash
CODEX_WEBVIEW_PORT=5180 make build-dev-app
```

### Port Integrations

Port integrations are build-time integration modules that adapt official Codex app
behavior and local runtime helpers to this Linux port. The source path is
`port-integrations/`.

This fork enables the current supported integration set by default: Open target
discovery, Agent Workspaces, AppShots, wrapper updater, Copilot reasoning effort
defaults, remote-control UI, mobile-control host patches, Read Aloud, Read Aloud
MCP, and conversation mode. Open target discovery lets the generated app discover
Linux terminals, editors, and file managers for the Open menus. Agent Workspaces
keeps its normal workspace start approval flow and permission controls in its
settings page; main-process hardening for direct bridge calls is tracked in
[#99](https://github.com/nisavid/codex-app-linux/issues/99). AppShots keeps
global hotkeys inactive until the user chooses one. Wrapper update checks stay
off at runtime until enabled in Settings.

To disable default integrations or enable still-optional integrations, copy
`port-integrations/integrations.example.json` to the git-ignored
`port-integrations/integrations.json`, edit the `enabled` and `disabled` lists, then
rebuild. Packaged installs can use
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/port-integrations.json` for the same
override shape; checkout builds ignore that persistent user file and use
`port-integrations/integrations.json` or `CODEX_PORT_INTEGRATIONS_CONFIG` instead. See
[`port-integrations/README.md`](../../port-integrations/README.md) for the integration
contract.

### Linux Computer Use UI Opt-In

The Linux Computer Use backend and plugin manifest are packaged by default. The
in-app UI controls are opt-in because they patch official OpenAI app bundle UI
paths during app generation.

Runtime readiness is separate from UI patching. Input synthesis usually
requires `ydotool`/`ydotoold`, `/dev/uinput` access, and a socket usable by your
desktop user. Non-GNOME desktops usually also need the matching XDG Desktop
Portal backend, such as the KDE or wlroots portal.

Treat Computer Use readiness as a set of independent paths. `doctor` can report
that AT-SPI, screenshots, window targeting, and input are available even when a
specific target app exposes only a sparse accessibility tree. In that case,
semantic actions may be unavailable while screenshots, window focus, and pointer
input still work.

Some non-GNOME sessions still use the historical
`org.gnome.desktop.interface toolkit-accessibility` setting to enable toolkit
accessibility bridges. The setting name does not imply that GNOME Shell is
required.

Raw key synthesis is evaluated by the active desktop layout after the backend
sends the key event. Non-QWERTY layouts, remapped keys, and custom Compose keys
can change both literal key output and shortcuts such as paste. For predictable
literal-key testing, record the current layout, switch temporarily to a standard
US/QWERTY layout, retry the action, then restore the original layout.

After building the app, inspect local readiness with:

```bash
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux doctor
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux setup
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux apps
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux windows
```

Enable the UI patches for one build:

```bash
CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1 make build-app
```

To keep the opt-in across updater rebuilds, write the persisted setting read by
the patcher at `${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/settings.json`. This
matters for updater runs because the `systemd --user` service does not inherit
interactive shell environment variables.

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

To remove the existing generated tree and redownload the DMG:

```bash
./install.sh --fresh
```

## Guided Native Setup

Run `make setup-native` when you want a host preflight summary before choosing
the build and install commands. The helper reports package-manager, desktop
session, package-format, updater, Computer Use, Read Aloud, and optional port
integration readiness. It can write `port-integrations/integrations.json`
for the next build, but it does not implicitly build, package, or install.

For non-interactive integration config:

```bash
CODEX_PORT_INTEGRATIONS=remote-mobile-control,read-aloud \
CODEX_DISABLE_PORT_INTEGRATIONS=conversation-mode \
CODEX_BOOTSTRAP_NONINTERACTIVE=1 \
make setup-native
```

To let the helper run the existing install flow, opt in explicitly:

```bash
CODEX_BOOTSTRAP_INSTALL_DEPS=1 \
CODEX_BOOTSTRAP_INSTALL_NATIVE=1 \
make setup-native
```

## Build Native Packages

Packaging scripts require `codex-app/` to exist. Run `make build-app` first.

Build the package type for the current host:

```bash
make package
```

Build a specific format:

```bash
make deb
make rpm
make pacman
make appimage
```

You can also run builders directly:

```bash
./scripts/build-deb.sh
./scripts/build-rpm.sh
./scripts/build-pacman.sh
./scripts/build-appimage.sh
```

Set `PACKAGE_WITH_UPDATER=0` when you need a native package that does not
install `codex-app-updater`, its `systemd --user` service, or the privileged
update support files:

```bash
PACKAGE_WITH_UPDATER=0 make package
PACKAGE_WITH_UPDATER=0 ./scripts/build-deb.sh
```

The legacy `PACKAGE_ENABLE_UPDATER=0` spelling is still accepted for older
local scripts, but new package commands should use `PACKAGE_WITH_UPDATER=0`.

By default, `install.sh` reads `Codex.app/Contents/Info.plist` from the
extracted DMG and writes `codex-app/codex-app-version.env`. Package builders use
that metadata, so an official OpenAI app bundle version such as
`26.422.30944 (2080)` becomes package version `26.422.30944`. Generated app
package versions use three or four numeric dot-separated segments so the updater
can compare installed and candidate versions consistently.

Override the package version only when you need to rebuild a known app tree with
an explicit local version:

```bash
PACKAGE_VERSION=26.422.30944 ./scripts/build-deb.sh
PACKAGE_VERSION=26.422.30944 ./scripts/build-rpm.sh
PACKAGE_VERSION=26.422.30944 ./scripts/build-pacman.sh
```

Expected outputs:

```text
dist/codex-app_<app-version>_<arch>.deb
dist/codex-app-<app-version>-1.<arch>.rpm
dist/codex-app-<app-version>-1-<arch>.pkg.tar.zst
dist/codex-app-<app-version>-<arch>.AppImage
```

Architecture names follow the package format: Debian uses `amd64`, `arm64`, or
`armhf`; RPM uses `x86_64`, `aarch64`, or `armv7hl`; pacman uses `x86_64` or
`aarch64`.

AppImages are manual-update artifacts. They omit `codex-app-updater`, the
systemd user service, polkit policy, and the native-package update-builder
bundle.

Native packages are named `codex-app`. They declare replacement metadata for
the older `codex-desktop` package name where the package format supports it,
while using the installed launcher and app layout at `/usr/bin/codex-app` and
`/opt/codex-app`.

Install the newest package in `dist/`:

```bash
make install
```

On Arch, direct installation also works:

```bash
sudo pacman -U dist/codex-app-*.pkg.tar.zst
```

## Updater Service

Native packages install `codex-app-updater` and its `systemd --user` service.
The service checks for newer official OpenAI Codex DMGs, rebuilds a local
native package, and uses privileged installation only for the final package
install.

Enable and start the service:

```bash
make service-enable
```

Inspect it:

```bash
make service-status
systemctl --user status codex-app-updater.service
codex-app-updater status --json
```

These targets make sense after installing a native package. A repo-only build
does not install the service unit or updater binary into the system.

## Make Targets

```bash
make help
make check
make test
make build-updater
make build-app
make run-app
make deb
make rpm
make pacman
make appimage
make package
make apple-dmg-verify
make release-gate
make install
make service-enable
make service-status
make clean
make clean-dist
make clean-state
```

`make appimage` builds a manual-update AppImage through
`./scripts/build-appimage.sh`; it consumes the generated `codex-app/` tree,
stages the AppDir templates under `packaging/appimage/`, and writes the
resulting `.AppImage` to `dist/`. `APPIMAGETOOL=/path/to/appimagetool` can
override the AppImage tool command. `make package` detects the native package
manager on the host and builds the matching package type. `make release-gate`
verifies the reviewed official OpenAI Codex DMG
hash, scans the generated app, validates package metadata, writes
`dist/SHA256SUMS`, and signs that checksum file whenever
`CODEX_RELEASE_GPG_KEY` is set. `REQUIRE_RELEASE_SIGNATURE=1` makes the gate
fail when that key is missing, which is the public-release mode. Signed gates
also publish `dist/release-signing-key.asc` and verify the signature against
that public key. `make install` installs the newest built native package.
`make clean` removes generated build artifacts: `codex-app/`, `Codex.dmg`, and
`dist/`. `make clean-state` removes updater runtime state under XDG directories.

## How The Build Works

The build flow is:

1. extract `Codex.dmg` with `7z` or `7zz`;
2. download or reuse the managed Linux Node.js runtime;
3. extract and patch `app.asar`;
4. rebuild native Node.js modules for Linux;
5. download a Linux Electron runtime;
6. write `codex-app/start.sh`;
7. optionally package `codex-app/` as a Debian, RPM, pacman, or AppImage
   artifact;
8. when installed from a native package, run `codex-app-updater` as a
   `systemd --user` service for local update checks and package rebuilds.

The macOS Codex app is an Electron application. Most of the app bundle is
platform-independent JavaScript, but the original package includes macOS-native
modules and a macOS Electron binary. The installer replaces Electron, rebuilds
native modules with `@electron/rebuild`, and removes the macOS-only `sparkle`
module. For the design rationale behind this flow, see
[Port Architecture](../port-architecture.md).

During ASAR patching, the installer also tries to adapt Linux window behavior:

- `Open in File Manager` integration is patched when the official OpenAI app
  bundle still matches the expected shape.
- If that targeted patch no longer matches, the installer continues and prints
  `Failed to apply Linux File Manager Patch`.
- Linux `opaqueWindows` defaults to `true` only when the user has not already
  saved an explicit `Translucent sidebar` preference.
