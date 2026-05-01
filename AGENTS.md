# AGENTS.md

## Purpose

This repository adapts the official macOS Codex App DMG to a runnable Linux build, packages that build as native `.deb`, `.rpm`, and pacman artifacts, and ships a local Rust update manager that rebuilds future Linux packages from newer upstream DMGs.

Treat this file as always-loaded agent policy. Keep detailed package recipes, runtime notes, and validation matrices in maintainer docs when those docs exist.

## Hard Rules

- `main` is protected. Before starting work, create and switch to a task branch.
- Commit completed work before handoff. For long tasks, also commit at staged,
  functional cutoff points. Each commit must pass the normal checks for the
  changed surface before it is created.
- Before pushing changes that affect the generated app, installer, ASAR patcher,
  package builders, package payload, updater rebuild flow, or bundled runtime
  helpers, run a local app generation/build gate first. The minimum gate is a
  successful `./install.sh` or `make build-app` from the current sources plus the
  relevant local package builder when package contents are affected. Refresh
  `Codex.dmg` first unless the cached DMG was refreshed within the last 24
  hours. If the host needs writable cache overrides such as `HOME`,
  `npm_config_cache`, or XDG paths, set them explicitly and record them in the
  verification notes.
- Use Conventional Commits. Commit messages must accurately describe the
  committed change.
- Do not hand-edit generated app output as the durable fix. Change `install.sh`,
  launcher templates, package templates, updater code, or shared helpers, then
  regenerate or inspect generated output as needed.
- Treat `codex-app/`, `codex-*-app/`, `dist/`, `Codex.dmg`, and XDG updater
  config/state/cache paths as generated or runtime artifacts unless the task
  explicitly targets them.
- Do not assume `codex-app/` is pristine. If it disagrees with source scripts,
  source scripts win.
- Keep Linux package behavior in `packaging/linux/`, `scripts/build-deb.sh`,
  `scripts/build-rpm.sh`, `scripts/build-pacman.sh`, and
  `scripts/lib/package-common.sh`.
- Preserve this fork's intentional names when syncing upstream: the app,
  install roots, launchers, package names, desktop files, and XDG app state use
  `codex-app`; the updater crate, binary, service, config, state, cache, and
  logs use `codex-app-updater`. Do not adopt upstream `codex-desktop` or
  `codex-update-manager` naming as part of a sync. Integrate upstream behavior
  under the local names instead.
- Preserve this fork's intentional layout when syncing upstream: native packages
  install under `/opt/codex-app`, user-local integration uses
  `~/.local/lib/codex-app`, and app state uses `~/.local/state/codex-app`.
  Do not adopt upstream `codex-app-linux` install roots as part of a sync.
- Keep native-package-only launcher behavior in
  `packaging/linux/codex-packaged-runtime.sh`; `install.sh` should stay generic
  and load that helper only when packaging requires it.
- Keep package builders and `scripts/lib/package-common.sh` aligned when adding,
  removing, or moving packaged files.
- Preserve the unprivileged updater boundary. Escalation belongs only at install
  time through the updater's privileged install subcommands.
- If the updater crate version changes, update `updater/Cargo.toml`,
  user-facing version references, and maintainer versioning docs in the same
  change.

The current working flow is:

1. `install.sh` extracts `Codex.dmg`
2. extracts and patches `app.asar`
3. rebuilds native Node modules for Linux
4. downloads a Linux Electron runtime
5. writes a Linux launcher into `codex-app/start.sh`
6. `scripts/build-deb.sh`, `scripts/build-rpm.sh`, or `scripts/build-pacman.sh` packages `codex-app/`
7. `codex-app-updater` runs as a `systemd --user` service and manages local auto-updates

## Source Of Truth

- `install.sh`
  Top-level installer entrypoint. Sources the `scripts/lib/*.sh` build-pipeline modules and emits `codex-app/start.sh` from the launcher template.
- `launcher/start.sh.template`
  Runtime launcher body. Concatenated by `install.sh::create_start_script` after a short prelude that bakes in the install-time app identity (`CODEX_LINUX_APP_ID`, display name, default webview port). Edit this file for any launcher behavior change — webview server lifecycle, warm-start handoff, CLI preflight, GUI prompts, URL-scheme handling, ydotool helpers.
- `scripts/lib/install-helpers.sh`
  Argument parsing, dependency checks, identity validation, install-dir preparation, color/log helpers, `shell_quote`.
- `scripts/lib/process-detection.sh`
  Running-app detection used to refuse overwriting a live install. Skips Electron utility helpers via `/proc/<pid>/cmdline` `--type=` heuristic.
- `scripts/lib/dmg.sh`
  DMG download, extraction, and Electron-version detection from upstream metadata.
- `scripts/lib/native-modules.sh`
  Native-module rebuild for Linux (`better-sqlite3`, `node-pty`) plus Electron download and cache.
- `scripts/lib/asar-patch.sh`
  Drives the Node patcher (`scripts/patch-linux-window-ui.js`) over `app.asar`.
- `scripts/lib/webview-install.sh`
  Webview asset extraction and final `codex-app/` install layout.
- `scripts/lib/bundled-plugins.sh`
  Linux Computer Use backend build, plugin staging, and bundled-plugin marketplace generation.
- `scripts/build-deb.sh`
  Builds the `.deb` from the already-generated `codex-app/`.
- `scripts/build-rpm.sh`
  Builds the `.rpm` from the already-generated `codex-app/`.
- `scripts/build-pacman.sh`
  Builds the `.pkg.tar.zst` from the already-generated `codex-app/`.
- `scripts/install-deps.sh`
  Installs host dependencies and bootstraps Rust.
- `scripts/lib/package-common.sh`
  Shared shell helpers used by the native package builders.
- `Makefile`
  Convenience targets for build, package, install, and cleanup workflows.
- `packaging/linux/control`
  Debian control template.
- `packaging/linux/codex-app.desktop`
  Desktop entry template.
- `packaging/linux/codex-packaged-runtime.sh`
  Packaged-launcher helper for native-package-only runtime behavior.
- `packaging/linux/codex-app.spec`
  RPM spec template.
- `packaging/linux/codex-app-updater.service`
  User-level `systemd` unit for the local update manager.
- `packaging/linux/codex-app-updater.prerm`
  Debian maintainer script that stops or disables the user service during removal.
- `packaging/linux/codex-app-updater.postrm`
  Debian maintainer script that reloads affected user managers after removal.
- `assets/codex.png`
  App icon used in native packages.
- `updater/`
  Rust crate that checks for new upstream DMGs, rebuilds local native-package artifacts, tracks update state, and installs prepared packages after the app exits.
- `updater/Cargo.toml`
  Source of truth for the updater crate version and dependency policy.
- `computer-use-linux/`
  Rust crate implementing the Linux Computer Use MCP backend (`codex-computer-use-linux` binary). Talks AT-SPI to read accessibility trees, captures screenshots through GNOME Shell DBus or XDG Desktop Portal, and synthesizes input via `ydotool`. Runs as a subprocess of Codex Electron when the bundled plugin is registered.
- `plugins/openai-bundled/plugins/computer-use/`
  Bundled plugin manifest for Linux Computer Use (`.codex-plugin/plugin.json` + `.mcp.json`). Author and license fields here must stay consistent with the repo's MIT license — they live alongside the runtime resources installed under `/opt/codex-app/resources/plugins/openai-bundled/`.
- `packaging/linux/codex-app-updater-user-service.sh`
  Shared shell helper sourced by `postinst` / `prerm` / `postrm` (DEB) and `%post` / `%preun` / `%postun` (RPM) plus pacman `.install` hooks. Provides `codex_ensure_user_service_running` / `codex_cleanup_user_service` / `codex_reload_user_managers` for safe `systemd --user` start/stop/disable across formats.
- `packaging/linux/com.github.nisavid.codex-app.update.policy`
  Polkit policy installed under `/usr/share/polkit-1/actions/` so the privileged updater install steps trigger the desktop authentication agent instead of `pkexec`'s textual fallback.
- `scripts/patch-linux-window-ui.js`
  ASAR patcher. Independent fail-soft patch functions with regex-driven needles. Each upstream-bundle change goes here.
- `scripts/patch-linux-window-ui.test.js`
  Node test suite for the patcher. Run with `node --test`.
- `docs/webview-server-evaluation.md`
  Decision record for the future Python-to-Rust webview server discussion.

## Generated Artifacts

- `codex-app/`
  Generated Linux app directory. Treat this as build output unless you are intentionally patching the launcher or testing package contents.
- `dist/`
  Generated packaging output, including `dist/codex-app_*.deb`, `dist/codex-app-*.rpm`, and `dist/codex-app-*.pkg.tar.zst`.
- `Codex.dmg`
  Cached upstream DMG. Useful for repeat installs.
- `~/.config/codex-app-updater/config.toml`
  Runtime config written or read by the updater service.
- `~/.local/state/codex-app-updater/state.json`
  Updater state machine persistence.
- `~/.local/state/codex-app-updater/service.log`
  Updater service log.
- `~/.cache/codex-app-updater/`
  Downloaded DMGs, rebuild workspaces, staged package artifacts, and build logs.

Do not assume `codex-app/` is pristine. If behavior differs from `install.sh`, prefer updating `install.sh` and then regenerating the app.

## Important Behavior And Known Fixes

- DMG extraction:
  `7z` can return a non-zero status for the `/Applications` symlink inside the DMG. This is currently treated as a warning if a `.app` bundle was still extracted successfully.
- Launcher and `nvm`:
  GUI launchers often do not inherit the user's shell `PATH`. The generated `start.sh` explicitly searches for `codex`, including common `nvm` locations.
- CLI preflight:
  Before Electron launches, the generated launcher asks `codex-app-updater` to verify the installed Codex CLI, prompt to install it when it is missing, and update it if the npm package is newer. Terminal launches prompt inline; GUI launches prefer `kdialog` on KDE/Plasma, otherwise `zenity`, before falling back to an actionable desktop notification. The check is best-effort: it uses a 1-hour cooldown for npm registry lookups, caches local CLI version reads to keep startup light, falls back to `npm install -g --prefix ~/.local` if a global install fails, and warns instead of blocking app launch when the refresh attempt does not succeed.
- ASAR patches are independent and fail-soft:
  `scripts/patch-linux-window-ui.js` is structured as a chain of small, independent patch functions called from `patchMainBundleSource`. Each one has its own regex-driven needles, an idempotency check, and a `console.warn` fall-back when the upstream bundle drifts. Current patches: `applyLinuxWindowOptionsPatch`, `applyLinuxMenuPatch`, `applyLinuxSetIconPatch`, `applyLinuxOpaqueBackgroundPatch`, `applyLinuxFileManagerPatch`, `applyLinuxTrayPatch`, `applyLinuxSingleInstancePatch`, `applyLinuxComputerUsePluginGatePatch`, `applyLinuxTrayCloseSettingPatch`, `applyLinuxSettingsPersistencePatch`, `applyLinuxLaunchActionArgsPatch`, `applyLinuxHotkeyWindowPrewarmPatch`, `applyBrowserAnnotationScreenshotPatch`. Plus `patchKeybindsSettingsAssets` (transactional — atomic, fail-soft via `WARN: Keybinds settings patch skipped: ...`) and `patchCommentPreloadBundle` for browser annotation fixes. When adding a new needle, mirror this pattern — never `throw`.
- Linux file manager integration:
  `applyLinuxFileManagerPatch` injects a Linux implementation for `Open in File Manager`. If the upstream minified bundle no longer matches, the install continues and emits exactly `Failed to apply Linux File Manager Patch`.
- Linux Computer Use plugin gate:
  `applyLinuxComputerUsePluginGatePatch` flips Codex's platform check from `darwin`-only to `darwin || linux` and adds `installWhenMissing: true` so the bundled plugin auto-registers. **Note:** the same feature is also gated by an OpenAI per-account Statsig rollout (`Qf('1506311413')` in the webview bundle, sets the `computerUse` feature flag). Installing the package only makes the platform side ready — the feature stays invisible in the UI until OpenAI flips the per-account flag. Same shape as the `gpt-5.5` model rollout. There is no project-side workaround that doesn't deliberately bypass OpenAI's gating; deferring that decision.
- Linux settings persistence:
  `applyLinuxSettingsPersistencePatch` inserts `codexLinuxPersistSettingsState(...)` so the keybinds-settings page toggles (system tray, warm start, compact prompt window) are mirrored to `~/.config/codex-app/settings.json`, where `linux_setting_enabled` in `install.sh` reads them. The patch is fail-soft: if the upstream `Yb` state-file marker or `set-global-state` IPC handler isn't present, the patch logs a warning and skips, leaving keybinds toggles in-memory only.
- Linux warm-start handoff:
  `applyLinuxLaunchActionArgsPatch` + `applyLinuxHotkeyWindowPrewarmPatch` add a Unix-domain-socket launch-action listener (`launch-action.sock` under `$XDG_RUNTIME_DIR/codex-app/`). When `start.sh` detects an existing Electron PID, it sends `--new-chat` / `--quick-chat` / `--prompt-chat` / `--hotkey-window` over the socket and exits, so a second launch never spawns a fresh Electron.
- Linux translucent sidebar default:
  During the same ASAR patch step, Linux defaults `Translucent sidebar` to `false` by applying `opaqueWindows: true` only when the app has no saved explicit value yet. This keeps existing user preferences intact while avoiding the sidebar disappearing bug on first run.
- Launcher logging:
  The generated launcher logs to:
  `~/.cache/codex-app/launcher.log`
- App liveness:
  The launcher writes a PID file to `~/.local/state/codex-app/app.pid`. The updater uses that plus `/proc` fallback to know whether Electron is still running.
- Desktop icon association:
  The launcher runs Electron with `--class=codex-app`, and the desktop file sets `StartupWMClass=codex-app` so the taskbar/dock can associate the correct icon.
- Webview server:
  The launcher starts a local `python3 -m http.server "$CODEX_LINUX_WEBVIEW_PORT"` from `content/webview/`, waits for the configured port to become reachable, verifies that `$WEBVIEW_ORIGIN/index.html` serves the expected Codex startup markers, and only then launches Electron because the extracted app expects local webview assets there.
- Wayland/GPU compatibility:
  The generated launcher enables `--ozone-platform-hint=auto`, `--disable-gpu-sandbox`, and `--enable-features=WaylandWindowDecorations` by default. Keep these in mind when debugging Pop!_OS, Wayland, or Nvidia-specific rendering issues.
- Webview server roadmap:
  Review `docs/webview-server-evaluation.md` before changing the local server model; that document captures the current recommendation, risks, and acceptance criteria.
- Closing behavior:
  If future work touches shutdown behavior, assume the confirmation dialog may be implemented inside the app bundle rather than the Linux launcher.
- Update manager:
  The native packages include `/usr/bin/codex-app-updater`, `/usr/lib/systemd/user/codex-app-updater.service`, and a minimal rebuild bundle under `/opt/codex-app/update-builder`.
- Privilege boundary:
  The updater runs unprivileged. It only escalates at install time via `pkexec /usr/bin/codex-app-updater install-deb --path <deb>`, `install-rpm --path <rpm>`, or `install-pacman --path <pkg.tar.zst>`.
- Failed privileged installs:
  A failed or cancelled `pkexec` install now stays in `Failed` and does not auto-retry every reconcile cycle. Check `service.log`, fix the root cause, and retry by waiting for the next rebuild or rebuilding a newer package.
- Interrupted installs:
  If updater state is left in `Installing` after a crash, restart, or interrupted privileged flow, the daemon now recovers that state automatically instead of staying stuck and skipping future upstream checks.
- Package removal:
  Debian and RPM removal now make a best-effort attempt to stop and disable `codex-app-updater.service` for active user sessions. If a user manager is unavailable, manual cleanup is still `systemctl --user disable --now codex-app-updater.service`.

## Crate Versioning

- Current updater crate version: `0.6.0`
- Bump `patch` for fixes, docs, and maintenance-only updates.
- Bump `minor` for compatible feature additions.
- Bump `major` for incompatible CLI, persisted-state, or install-flow changes.
- If the updater crate version changes, update README and AGENTS in the same change so the maintenance docs do not drift.

## How To Rebuild

### Regenerate the Linux app

```bash
./install.sh ./Codex.dmg
```

Or let the script download the DMG:

```bash
./install.sh
```

### Build the Debian package

```bash
./scripts/build-deb.sh
```

Default output:

```bash
dist/codex-app_YYYY.MM.DD.HHMMSS_amd64.deb
```

Optional version override:

```bash
PACKAGE_VERSION=2026.03.24.120000+deadbeef ./scripts/build-deb.sh
```

### Build the RPM package

```bash
./scripts/build-rpm.sh
```

Default output:

```bash
dist/codex-app-YYYY.MM.DD.HHMMSS-<release>.x86_64.rpm
```

Optional version override:

```bash
PACKAGE_VERSION=2026.03.24.120000+deadbeef ./scripts/build-rpm.sh
```

### Build the pacman package

```bash
./scripts/build-pacman.sh
```

Default output:

```bash
dist/codex-app-YYYY.MM.DD.HHMMSS-<release>-x86_64.pkg.tar.zst
```

Optional version override:

```bash
PACKAGE_VERSION=2026.03.24.120000+deadbeef ./scripts/build-pacman.sh
```

## Runtime Expectations

- `node`, `npm`, `npx`, `python3`, `7z`, `curl`, `unzip`, `make`, and `g++` are required for `install.sh`
- Node.js 20+ is required
- On apt-based systems, `scripts/install-deps.sh` uses a compatible distro `nodejs`/`npm` candidate when available and otherwise bootstraps NodeSource Node.js 22 by default. `NODEJS_MAJOR=24 bash scripts/install-deps.sh` selects Node.js 24 instead.
- the packaged app still requires the Codex CLI at runtime:
  `codex` must exist in `PATH` or be set through `CODEX_CLI_PATH`, but the launcher now attempts a best-effort automatic install on first run when the CLI is missing and `npm` is available

## Packaging Notes

The native packages currently install:

- app files under `/opt/codex-app`
- launcher under `/usr/bin/codex-app`
- updater binary under `/usr/bin/codex-app-updater`
- updater unit under `/usr/lib/systemd/user/codex-app-updater.service`
- update builder bundle under `/opt/codex-app/update-builder`
- desktop file under `/usr/share/applications/codex-app.desktop`
- icon under `/usr/share/icons/hicolor/256x256/apps/codex-app.png`

The Debian builder uses `dpkg-deb --root-owner-group` so package ownership is correct.

The RPM builder stages the same app and updater payload into an RPM buildroot before invoking `rpmbuild`.

The pacman builder stages the same payload into a package root, writes `.PKGINFO`/`.MTREE`, and then produces a `.pkg.tar.zst` archive for `pacman -U`.

## Preferred Validation After Changes

After editing installer or packaging logic, validate at least:

```bash
bash -n install.sh
bash -n scripts/lib/*.sh
bash -n launcher/start.sh.template
bash -n scripts/build-deb.sh
bash -n scripts/build-rpm.sh
bash -n scripts/build-pacman.sh
cargo check -p codex-app-updater
cargo test -p codex-app-updater
./scripts/build-deb.sh
dpkg-deb -I dist/codex-app_*.deb
dpkg-deb -c dist/codex-app_*.deb | sed -n '1,40p'
```

Treat the local app generation/build as a push gate for changes that affect
`install.sh`, `scripts/patch-linux-window-ui.js`, `scripts/lib/*.sh`,
`launcher/start.sh.template`, package builders/templates, updater package
rebuild behavior, or bundled runtime payloads. Do not push those changes based
only on static checks or CI package jobs; first prove the current checkout can
generate `codex-app/` locally from a `Codex.dmg` refreshed within the last 24
hours, then build the affected package format locally when package contents
changed.

If `rpmbuild` is available, also run:

```bash
./scripts/build-rpm.sh
```

If `pacman` is available, also run:

```bash
./scripts/build-pacman.sh
pacman -Qip dist/codex-app-*.pkg.tar.zst
pacman -Qlp dist/codex-app-*.pkg.tar.zst | sed -n '1,40p'
```

If launcher behavior changed, also inspect:

```bash
sed -n '1,120p' codex-app/start.sh
```

If updater behavior changed, also inspect:

```bash
systemctl --user status codex-app-updater.service
codex-app-updater status --json
sed -n '1,120p' ~/.local/state/codex-app-updater/state.json
sed -n '1,160p' ~/.local/state/codex-app-updater/service.log
```

## Editing Guidance

- Prefer changing `launcher/start.sh.template` (for runtime/launcher behavior) or `scripts/lib/*.sh` (for build-pipeline behavior) over manually patching `codex-app/start.sh`, unless you are making a temporary local test. `install.sh` itself stays small — it's just orchestration and the prelude that bakes install-time identity into the generated launcher.
- Keep native-package-only launcher behavior in `packaging/linux/codex-packaged-runtime.sh`; `launcher/start.sh.template` should stay generic and only load that helper optionally.
- If you update `launcher/start.sh.template`, regenerate `codex-app/` or keep `codex-app/start.sh` aligned before building a new package.
- Keep packaging changes in `packaging/linux/`, `scripts/build-deb.sh`, `scripts/build-rpm.sh`, and `scripts/build-pacman.sh`; avoid hardcoding distro-specific behavior outside those files unless necessary.
- Keep `scripts/lib/package-common.sh` aligned with both builders when you add or remove packaged files from the shared runtime payload.
