# Build and Run Guide

This guide is for users who want to run Codex on Linux or build a native
package from this repository.

## Prerequisites

You need:

- Node.js 20 or newer;
- `npm` and `npx`;
- `python3`;
- `7z` or `7zz`;
- `curl`;
- `unzip`;
- `make`;
- `g++` or equivalent C++ build tooling;
- Rust and `cargo` for `codex-app-updater`.

The dependency helper supports `apt`, `dnf5`, `dnf`, and `pacman`:

```bash
bash scripts/install-deps.sh
```

The generated launcher can install `@openai/codex` on first run when the CLI is
missing and `npm` is available. To install it before launching:

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

It installs Node.js, npm, Python, 7z, curl, build tools, and bootstraps Rust
through `rustup` if `cargo` is missing.

### Arch Linux

Run the dependency helper:

```bash
bash scripts/install-deps.sh
```

Or install the system packages directly:

```bash
sudo pacman -S --needed nodejs npm python p7zip curl unzip zstd base-devel
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

The flake pins the SRI hash of the upstream `Codex.dmg`. OpenAI republishes the
DMG at the same URL for each release, so the hash can temporarily lag. A GitHub
Actions job refreshes the hash on `main` once every 24 hours. If you see:

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

To remove the existing generated tree and redownload the DMG:

```bash
./install.sh --fresh
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
```

You can also run builders directly:

```bash
./scripts/build-deb.sh
./scripts/build-rpm.sh
./scripts/build-pacman.sh
```

By default, `install.sh` reads `Codex.app/Contents/Info.plist` from the
extracted DMG and writes `codex-app/codex-app-version.env`. Package builders use
that metadata, so an upstream app version such as `26.422.30944 (2080)` becomes
package version `26.422.30944.2080`. Generated app package versions are numeric
dot-separated segments so the updater can compare installed and candidate
versions consistently.

Override the package version only when you need to rebuild a known app tree with
an explicit local version:

```bash
PACKAGE_VERSION=26.422.30944.2080 ./scripts/build-deb.sh
PACKAGE_VERSION=26.422.30944.2080 ./scripts/build-rpm.sh
PACKAGE_VERSION=26.422.30944.2080 ./scripts/build-pacman.sh
```

Expected outputs:

```text
dist/codex-app_<upstream-version>_amd64.deb
dist/codex-app-<upstream-version>-1.x86_64.rpm
dist/codex-app-<upstream-version>-1-x86_64.pkg.tar.zst
```

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
The service checks for newer upstream DMGs, rebuilds a local native package, and
uses privileged installation only for the final package install.

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
make package
make install
make service-enable
make service-status
make clean-dist
make clean-state
```

`make package` detects the native package manager on the host and builds the
matching package type. `make install` installs the newest built native package.

## How The Build Works

The build flow is:

1. extract `Codex.dmg` with `7z` or `7zz`;
2. extract and patch `app.asar`;
3. rebuild native Node.js modules for Linux;
4. download a Linux Electron runtime;
5. write `codex-app/start.sh`;
6. optionally package `codex-app/` as a Debian, RPM, or pacman package;
7. when installed from a native package, run `codex-app-updater` as a
   `systemd --user` service for local update checks and package rebuilds.

The macOS Codex app is an Electron application. Most of the app bundle is
platform-independent JavaScript, but the original package includes macOS-native
modules and a macOS Electron binary. The installer replaces Electron, rebuilds
native modules with `@electron/rebuild`, and removes the macOS-only `sparkle`
module.

During ASAR patching, the installer also tries to adapt Linux window behavior:

- `Open in File Manager` integration is patched when the upstream bundle still
  matches the expected shape.
- If that targeted patch no longer matches, the installer continues and prints
  `Failed to apply Linux File Manager Patch`.
- Linux `opaqueWindows` defaults to `true` only when the user has not already
  saved an explicit `Translucent sidebar` preference.
