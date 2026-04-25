# User-Local App Integration

This directory contains an experimental rootless install path for users who do
not want to install a native system package.

It installs Codex support files under `~/.local`, creates convenient
wrappers in `~/.local/bin`, and can opt in to a weekly user-level update timer.
Use the native packages when you want the supported package-manager path; use
this layout when you want a self-contained per-user install.

## What It Installs

The installer creates:

- `~/.local/lib/codex-app/` as the stable private install root;
- helper scripts under `~/.local/lib/codex-app/bin/`;
- thin wrappers under `~/.local/bin/`;
- a desktop entry under `~/.local/share/applications/`;
- user `systemd` unit files under `~/.config/systemd/user/`;
- metadata under `~/.local/state/codex-app/` that records the source checkout
  and cached `Codex.dmg`;
- an icon extracted from the local `Codex.dmg` when one is available.

The helper scripts are copied into `~/.local/lib/codex-app`; they do not run
directly from the git checkout. They record the checkout path in user state,
then use that checkout for future `git pull` operations while rebuilding
runtime assets into the user-local install root.

The preferred checkout location is:

```text
~/workspace/codex-app-linux
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

The timer runs `codex-app-update --quiet`. It is opt-in; you can also enable
it later:

```bash
systemctl --user enable --now codex-app-update.timer
```

## Commands

After installation, these wrappers should be on `PATH` if `~/.local/bin` is
included in your shell environment:

```bash
codex-app
codex-app-check-update
codex-app-update
codex-app-version
```

## File Layout Reference

Reusable payload files in this directory are copied into:

```text
~/.local/lib/codex-app/bin/
~/.local/lib/codex-app/lib/
~/.local/bin/
~/.local/share/applications/
~/.local/state/codex-app/
~/.config/systemd/user/
```

The desktop entry source lives at
`files/.local/share/applications/codex-app.desktop`. The optional timer and
service sources live at:

```text
files/.config/systemd/user/codex-app-update.service
files/.config/systemd/user/codex-app-update.timer
```

## Notes

- The icon is generated locally from `Codex.dmg`; it is not committed here as a
  binary asset.
- The helper scripts check both wrapper repository changes and upstream
  `Codex.dmg` headers.
- The user-local path is separate from the native `codex-app-updater`
  package service.
