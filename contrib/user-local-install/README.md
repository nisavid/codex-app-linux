# User-Local Desktop Integration

This directory contains an experimental rootless install path for users who do
not want to install a native system package.

It installs Codex Desktop support files under `~/.local`, creates convenient
wrappers in `~/.local/bin`, and can opt in to a weekly user-level update timer.
Use the native packages when you want the supported package-manager path; use
this layout when you want a self-contained per-user install.

## What It Installs

The installer creates:

- `~/.local/opt/codex-desktop-linux/` as the stable install root;
- helper scripts under `~/.local/opt/codex-desktop-linux/bin/`;
- thin wrappers under `~/.local/bin/`;
- a desktop entry under `~/.local/share/applications/`;
- user `systemd` unit files under `~/.config/systemd/user/`;
- metadata that records the source checkout and cached `Codex.dmg`;
- an icon extracted from the local `Codex.dmg` when one is available.

The helper scripts are copied into `~/.local/opt`; they do not run directly from
the git checkout. They record the checkout path in user state, then use that
checkout for future `git pull` operations while rebuilding runtime assets into
the user-local install root.

The preferred checkout location is:

```text
~/workspace/codex-desktop-linux
```

Other checkout paths work, but the recorded path must remain available for
update checks.

## Install

From the repository root:

```bash
./contrib/user-local-install/install-user-local.sh
```

Enable the weekly auto-update timer during installation:

```bash
./contrib/user-local-install/install-user-local.sh --enable-timer
```

The timer runs `codex-desktop-update --quiet`. It is opt-in; you can also enable
it later:

```bash
systemctl --user enable --now codex-desktop-update.timer
```

## Commands

After installation, these wrappers should be on `PATH` if `~/.local/bin` is
included in your shell environment:

```bash
codex-desktop
codex-desktop-check-update
codex-desktop-update
codex-desktop-version
```

## File Layout Reference

Reusable payload files in this directory are copied into:

```text
~/.local/opt/codex-desktop-linux/bin/
~/.local/opt/codex-desktop-linux/lib/codex-desktop-linux/
~/.local/bin/
~/.local/share/applications/
~/.config/systemd/user/
```

The desktop entry source lives at
`files/.local/share/applications/codex-desktop.desktop`. The optional timer and
service sources live at:

```text
files/.config/systemd/user/codex-desktop-update.service
files/.config/systemd/user/codex-desktop-update.timer
```

## Notes

- The icon is generated locally from `Codex.dmg`; it is not committed here as a
  binary asset.
- The helper scripts check both wrapper repository changes and upstream
  `Codex.dmg` headers.
- The user-local path is separate from the native `codex-update-manager`
  package service.
