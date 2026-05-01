# Fork Divergences

This maintainer inventory records intentional contracts in this fork that need
explicit review during upstream syncs. Keep the list current when a change adds,
removes, or relocates a fork-specific behavior.

## Identity

- The Linux app, package, launcher, desktop entry, icon name, app state, and
  package metadata use `codex-app`.
- The updater crate, updater binary, user service, XDG config, state, cache, and
  logs use `codex-app-updater`.
- Compatibility references to older names are allowed only where they preserve
  upgrade or removal behavior, such as package `provides`, `conflicts`,
  replacement metadata, or one-time cleanup of `codex-update-manager.service`.

## Versioning

- Native package versions come from the OpenAI DMG app bundle metadata.
  `install.sh` writes `codex-app/codex-app-version.env`, and package builders
  default to `CODEX_APP_PACKAGE_VERSION` from that file.
- Generated package versions must stay three or four numeric dot-separated
  segments so updater comparisons work across package formats.
- Local timestamp or commit-hash versions are suitable only for deliberate test
  builds with an explicit `PACKAGE_VERSION` override.

## Runtime And Updater Behavior

- Native packages install the generated Electron app under this fork's package
  name and expose `/usr/bin/codex-app`.
- The generated launcher loads packaged-only behavior from the generated app
  tree only when that helper is present, so checkout installs remain generic.
- The packaged runtime helper imports desktop/session display variables into
  the user systemd manager, but does not import the user session `PATH`.
- The packaged runtime helper disables the legacy `codex-update-manager.service`
  name when present, then enables or starts `codex-app-updater.service` on a
  best-effort basis without restarting an already active updater service.
- Launch-time update checks run in the background after the launcher records the
  Electron PID.
- Codex CLI discovery uses explicit command options, `CODEX_CLI_PATH`, updater
  config `cli_path`, persisted updater state, launch `PATH`, then known
  user-local package-manager paths.
- The updater's production builder bundle cannot be redirected by runtime config
  unless `developer_mode = true` is set explicitly.
- The updater remains unprivileged until package installation. Privileged work
  is limited to `install-deb`, `install-rpm`, and `install-pacman` subcommands.

## Package And Supply Chain Hardening

- Package staging validates generated app payload symlinks and normalizes app
  payload modes before native package creation.
- Pacman package staging does not preserve source ownership, so packaged payload
  ownership is determined by package manager installation rather than by local
  build artifacts.
- Privileged install subcommands reject symlink and non-file candidates, require
  expected package filename shapes, copy the package into a private temporary
  staging directory, and validate package identity metadata before installing.
- Production builder roots must not be symlinks or group/world-writable. They
  must be owned by root, or by the kernel overflow UID when a user namespace
  exposes root-owned package files that way.
- Upstream DMGs should be refreshed before local build gates when the cached
  `Codex.dmg` is older than 24 hours.

## Layout Triage

These path decisions are triaged against the XDG Base Directory Specification,
the Filesystem Hierarchy Standard, and common distro packaging conventions for
Electron-style app bundles.

| Surface | Decision | Rationale |
| --- | --- | --- |
| Generated native app bundle | Keep `/opt/codex-app`. | The extracted DMG/Electron tree is a self-contained add-on app bundle. `/opt/<package>` is the conventional location for that shape. |
| User-facing launchers | Keep `/usr/bin/codex-app` and `/usr/bin/codex-app-updater`. | Package-managed commands belong on the normal system command path. |
| Update builder bundle | Use `/usr/lib/codex-app/update-builder`. | The builder is package-private support used by `codex-app-updater`, not part of the app bundle or user data. |
| Packaged runtime helper | Use `/usr/lib/codex-app/packaged-runtime.sh`. | The helper is package-private launcher support sourced by the generated launcher only in native package installs. |
| Desktop entry and icon | Keep `/usr/share/applications/codex-app.desktop` and `/usr/share/icons/hicolor/256x256/apps/codex-app.png`. | Freedesktop desktop integration is shared, package-managed data. |
| Updater config, state, cache, logs | Keep XDG paths: `~/.config/codex-app-updater`, `~/.local/state/codex-app-updater`, `~/.cache/codex-app-updater`, and `~/.cache/codex-app/launcher.log`. | These are per-user mutable files and should follow XDG base directories. |
| App PID, webview PID, launch-action socket | Keep `~/.local/state/codex-app` for persistent state and `$XDG_RUNTIME_DIR/codex-app` for runtime sockets when available. | Persistent restart state belongs in XDG state; sockets and runtime objects belong in XDG runtime. |
| User-local non-package app payloads | Use `${XDG_DATA_HOME:-~/.local/share}/codex-app`. Do not use `~/.local/opt`. | XDG has no `~/.local/opt`; user-specific app data should start from the XDG data base directory. |

No path ambiguity remains for the native package payload after this triage. The
experimental unprivileged install uses XDG user paths and should stay aligned
with this table unless a more specific distro convention is adopted
deliberately.
