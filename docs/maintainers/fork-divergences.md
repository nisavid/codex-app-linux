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

## Open Layout Triage

These path decisions are intentionally unresolved here. During this upstream
sync, do not rewrite one side into the other without maintainer triage.

| Surface | Current source/code references | Conflicting doc or upstream-sync references | Triage question |
| --- | --- | --- | --- |
| Update builder bundle | `scripts/lib/package-common.sh` stages `/opt/<package>/update-builder`; updater defaults to `/opt/codex-app/update-builder`; package maintainer scripts source helpers from `/opt/codex-app/update-builder/...`. | `docs/maintainers/package-runtime-maintenance.md` still describes `/usr/lib/codex-app/update-builder`. | Should the packaged update-builder live under `/opt/codex-app` with the app payload, or under `/usr/lib/codex-app` as architecture-independent support data? |
| Packaged runtime helper | `launcher/start.sh.template` loads `$SCRIPT_DIR/.codex-linux/codex-packaged-runtime.sh`; package staging renders the helper into `/opt/<package>/.codex-linux/`. | `docs/maintainers/package-runtime-maintenance.md` still describes `/usr/lib/codex-app/packaged-runtime.sh`. | Should packaged-only launcher behavior stay beside the generated app tree, or move to `/usr/lib/codex-app`? |
| User-local checkout install root | `AGENTS.md` states user-local integration uses `~/.local/lib/codex-app`. | The rejected path family was `~/.local/opt/...`; upstream syncs may reintroduce it through installer or docs. | Should all user-local app payload references standardize on `~/.local/lib/codex-app`? |
| App payload root | Package code and user docs use `/opt/codex-app` for the generated Electron app. | Some support payload decisions above may move non-app files to `/usr/lib/codex-app`. | Which files are app payload and which are package support payload? |

