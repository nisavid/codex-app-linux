# Changelog

All notable changes to this project are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Fixed

- Nix builds, installer apps, and dev shells now use modern `7zz`, and the installer dependency check accepts `7zz` without requiring a separate legacy `7z` binary.
- Codex App no longer removes user-enabled `remote_control = true` from the local Linux config before starting the app server.

## [0.8.0] - 2026-05-16

### Added

- New local AppImage build target for manual-update installs on systems where native packages are awkward.
- Opt-in multi-instance launcher support via `--new-instance` or `CODEX_MULTI_LAUNCH=1`, with isolated Electron profiles, pid files, sockets, logs, and bounded webview ports.
- New opt-in Linux features `remote-control-ui` and `remote-mobile-control` that patch upstream remote-control surfaces on Linux without faking backend state, MFA, or connected-client data.
- `linux-features` can now contribute opt-in `webview-asset` patch descriptors in addition to main-bundle patches, so feature-scoped Linux UI experiments can hook hashed webview assets without being promoted into the core patch registry.
- Nix outputs for the default app, Computer Use UI, remote mobile control, the combined feature set, and the installer.

### Fixed

- Updater and launcher edge cases around interrupted installs, persisted Linux features, user-local managed checkout metadata, webview origin probes behind proxy settings, and package metadata.
- Pacman builds now expose a `codex-app-latest.pkg.tar.zst` symlink for the most recent local package artifact.
- The webview patcher can keep the composer rate-limit footer visible across upstream bundle drift.

## [0.7.1] - 2026-05-06

### Fixed

- The Chrome native-messaging host now evicts stale browser clients when a newer Codex browser client connects, preventing old Node REPL sessions from repeatedly reattaching CDP and driving extension service-worker CPU.
- The bundled Chrome plugin is now auto-installed during app startup, matching Browser Use, so the plugin page no longer falls back to an install button after restart when the Linux native host is already staged.
- Local auto-update rebuilds and package builds now find the Rust toolchain reliably when `cargo` is installed via `rustup` under `~/.cargo/bin`, even if the `codex-app-updater` user service or packaging scripts inherit a reduced `PATH`.

### Added

- Regression coverage for the build-environment fix: updater path construction now has a unit test for `~/.cargo/bin`, and packaging helpers resolve `cargo` through the same fallback path used by Linux Computer Use staging.

## [0.7.0] - 2026-05-04

### Added

- Linux Computer Use plugin now exposes accessibility actions and editable-value setting via a new `perform_action` MCP tool. `element_index` selections resolve to AT-SPI object-reference IDs from the latest `get_app_state` result, then open the target object on demand so actions and value writes target the same node as a click.
- UI-driven Linux app update flow: when an update is rebuilt and ready, the in-app updater control can request install. The app exits, the user service installs the package, and the launcher relaunches `/usr/bin/codex-app` after the update lands. Backed by a new `codex-app-updater install-ready` subcommand and a `scripts/rebuild-candidate.sh` helper packaged into the update-builder bundle.
- NixOS launcher exposes Electron GL/EGL libraries and primary-runtime native libraries via `LD_LIBRARY_PATH`, so the bundled Python/Node payloads (Pillow, NumPy, sharp, canvas) load on stock NixOS.
- The installer now bundles a managed Linux Node.js 22.22 runtime into `codex-app/resources/node-runtime`; packaged launches and local auto-update rebuilds use it before any system, nvm, asdf, or manually installed Node.js.

### Changed

- `get_app_state(window_id=...)` and `get_app_state(pid=...)` prefer exact PID/window-root matching when resolving the AT-SPI tree.
- `click(element_index=...)` falls back to the primary AT-SPI action when the element exposes no usable bounds.
- `app.asar` repack is now reproducible: file ordering is sorted and `node-pty/build/Makefile` (which embeds absolute build paths) is removed before packing.
- Native packages no longer hard-depend on distro `nodejs`/`npm`; the bundled managed Node.js runtime covers Codex CLI install/update flows, Browser Use, and updater rebuilds. This lets users with `nvm`, asdf, Volta, or nodejs.org tarball installs install the native package cleanly. Fixes #104.

### Fixed

- AT-SPI sentinel bounds no longer trigger bogus portal clicks on hidden or off-screen nodes.
- Linux quit now bypasses the close-to-tray gate so the app actually exits instead of getting trapped in the tray.
- Keybinds settings index patch tolerates upstream minified variable-name drift; the route map is detected via a `(0,X.lazy)` lookahead instead of hard-coded `c_e` / `Xge` / `Zge` names.
- NixOS-installed `start.sh` shebang is patched to a nix-store `bash` so the launcher actually runs on systems without `/bin/bash`.
- Native packages now always stage `scripts/lib/node-runtime.sh` into `/usr/lib/codex-app/update-builder`, so local auto-update rebuilds can source the managed Node runtime helper instead of failing before package generation.

## [0.6.2] - 2026-05-01

### Changed

- Merged upstream launcher and updater changes: Electron now targets `41.3.0`,
  launcher startup can reuse an existing verified webview server for warm
  starts, launcher CLI checks use `codex-app-updater cli-preflight`, packaged
  launch-time update checks use `codex-app-updater check-now --if-stale`, and
  RPM installs can use `zypper` on openSUSE.
- Native package versions now default to the upstream Codex app bundle marketing
  version from `Contents/Info.plist` (`CFBundleShortVersionString`). Generated
  package metadata is restricted to three or four numeric dot-separated segments
  so updater comparisons stay consistent across package formats.
- Missing Codex CLI recovery is exposed as an explicit
  `cli_status: NotInstalled` state in updater status output and persisted state,
  instead of overloading `Unknown`.
- Automatic installation of a missing Codex CLI is launcher-scoped. The daemon
  and `codex-app-updater status` report and notify when the dependency is
  missing, but they do not install missing CLI dependencies on their own;
  best-effort reconciliation may still upgrade an existing CLI.

### Added

- Linux Computer Use UI patches are now opt-in through
  `CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1` at build time or
  `~/.config/codex-app/settings.json` with
  `"codex-linux-computer-use-ui-enabled": true`, including smoke coverage for
  default-off, env-var, and persisted-setting paths.

- The bundled Computer Use plugin manifest gate remains default-on for Linux
  platform registration, while the UI patches that touch upstream rollout-gated
  code now require explicit opt-in.

### Fixed

- Launcher error messages now distinguish between a CLI that is missing versus an automatic installation attempt that failed, clarifying the supported recovery path.
- Missing-CLI desktop notifications now key off the explicit `NotInstalled` state instead of inferring absence from cleared fields.
- Restored Makefile help and targets for `clean`, `apple-dmg-verify`, and
  `release-gate`, and kept package-build examples on the default
  DMG-derived version path.

## [0.6.1] - 2026-04-30

### Added

- GitHub Actions workflow `upstream-build-app.yml` builds `make build-app`
  against the real upstream `Codex.dmg`, records DMG provenance, and exercises
  the installer/cache behavior against current upstream metadata.

### Changed

- Script smoke tests now cover the upstream-DMG build workflow and newer ASAR
  patcher shapes, including launch-action, tray, Computer Use, and stale
  background-helper cases.

## [0.6.0] - 2026-04-30

### Added

- Packaged GUI launches can now prompt to install a missing Codex CLI through `codex-app-updater`, preferring `kdialog` on KDE/Plasma, then `zenity`, and finally an actionable desktop notification when no dialog helper is available.
- `scripts/install-deps.sh` now installs one desktop-appropriate GUI dialog helper so first-run CLI installation works cleanly outside a terminal.
- GitHub Actions CI now runs Rust checks, script smoke tests, and real Debian, RPM, and pacman package build validations with job summaries.

### Changed

- `make build-app` now defers to `install.sh` when no `DMG=...` override is provided, so fresh checkouts can reuse or download `Codex.dmg` through the installer's normal flow instead of failing on a missing local cache path.
- Electron runtime downloads are now cached under `~/.cache/codex-app/electron` and resume interrupted transfers, reducing repeated `make build-app` rebuild time.
- Launcher CLI preflight now uses cached local CLI state on the fast path, leaving heavier `codex --version` and registry refresh work to the updater when the cache is stale or invalid.

### Fixed

- `make build-app` now rebuilds `better-sqlite3` with an Electron 41-compatible release when the upstream DMG bundles an older native module source.
- `codex-app-updater` now refreshes CLI status when the daemon starts and shows a desktop notification if the Codex CLI is missing, so package installs do not rely on the user manually checking updater state to understand why Codex App cannot launch cleanly.
- When the Codex CLI is missing, terminal launches still prompt before installation and GUI launches now have a matching fallback path instead of failing with only a passive notification.

## [0.5.0] - 2026-04-30

### Added

- Linux Computer Use plugin and native Rust MCP backend `codex-computer-use-linux`. Provides AT-SPI accessibility-tree access, screenshot capture through GNOME Shell or XDG Desktop Portal, and `ydotool` input synthesis. Plugin is gated by OpenAI's per-account Statsig rollout (`computerUse` feature flag), so installing the package does not by itself make Computer Use appear in the Codex UI.
- Linux keybinds settings page injected into the Codex webview, with persistent toggles for the compact prompt window, system tray, and warm-start handoff.
- Warm-start handoff: launching the app while another instance is already running now sends the launch action over a Unix-domain socket (`launch-action.sock`) and exits, instead of starting a fresh Electron. New launcher CLI flags `--new-chat`, `--quick-chat`, `--prompt-chat`, `--hotkey-window` route through that path.
- Linux system tray with platform-gated guard, single-instance lock, and second-instance window focus through Electron's `requestSingleInstanceLock` / `second-instance` event.
- Polkit policy `com.github.nisavid.codex-app.update.policy` so privileged updater installs use the desktop authentication agent (`pkexec --disable-internal-agent`) instead of falling back to a textual prompt.
- openSUSE / zypper support across `scripts/install-deps.sh`, the `make install` target, and the updater's RPM install path.
- Browser Use bundled plugin resources are now installed alongside the Linux app, with launcher-side environment hydration for `CODEX_ELECTRON_RESOURCES_PATH`, `CODEX_BROWSER_USE_NODE_PATH`, and `CODEX_NODE_REPL_PATH`.
- Apt Node bootstrap: `install-deps.sh` prefers a compatible distro `nodejs`/`npm` candidate and otherwise installs Node.js 22 from NodeSource. CI matrix validates the bootstrap on Ubuntu 22.04, Ubuntu 24.04, and Debian 12.
- Electron version is now auto-detected from upstream DMG metadata (`Electron Framework.framework/Versions/A/Resources/Info.plist` then `app.asar` `package.json`); the pinned `41.3.0` remains as the fallback when detection fails.
- `codex-app-updater check-now --if-stale` subcommand and a launch-time best-effort check that skips when the last successful upstream check is still fresh.
- New updater subcommand `prompt-install-cli` plus persisted-state field `cli_last_verified_at` to support GUI-launched CLI install prompts and a cached-status fast path.

### Changed

- ASAR patcher refactored into independent fail-soft patch functions with regex-driven needles instead of hard-coded minified variable names. Added Node test suite (`scripts/patch-linux-window-ui.test.js`).
- DEB / RPM / pacman packages now declare `nodejs (>= 20)` and pull in `polkit` or legacy policykit package names where needed, so the privileged install flow works out of the box on supported distros.
- Wayland sessions with `DISPLAY` available now default to `--ozone-platform=x11` for Electron popup positioning compatibility; pure Wayland sessions keep `--ozone-platform-hint=auto`.
- RPM `%preun` only stops and disables the user updater service on package erase (`$1 -eq 0`), not on upgrade.
- RPM staging now uses the shared `stage_common_package_files` / `stage_update_builder_bundle` helpers, fixing missing packaged runtime helper and update-builder payload files in shipped RPMs.
- Updater check serialization moved to a kernel-backed file lock (`flock(2)` via the `fs4` crate). A non-graceful exit no longer leaves a stale sentinel file that silences future upstream checks.
- Webview server is now adopted and reused across launches instead of being killed and restarted, and explicitly binds to `127.0.0.1` only.

### Fixed

- Failed `pkexec` authentication now keeps the candidate ready for retry on the next app exit instead of surfacing repeat prompts every reconcile cycle.
- RPM installs now reject non-newer package versions, matching the existing DEB and pacman downgrade guards.
- Linux browser annotation screenshots now use the stored anchor geometry and render only the selected marker, fixing misaligned and over-cluttered annotation captures.
- The Linux settings persistence patch now warns and skips instead of throwing when its needle is missing on a fresh upstream bundle, so the install pipeline no longer aborts on a bundle-shape change.
- DEB packages now alternate-depend on legacy policykit package names where needed, so installs succeed on older Debian/Ubuntu-derived systems.

## [0.4.2] - 2026-04-23

### Changed

- `make build-app` now defers to `install.sh` when no `DMG=...` override is provided, so fresh checkouts can reuse or download `Codex.dmg` through the installer's normal flow instead of failing on a missing local cache path.
- Launcher CLI preflight now uses `install.sh` to decide when missing-CLI installation is allowed, instead of always enabling the updater's auto-install path up front.

### Fixed

- `codex-app-updater` now refreshes CLI status when the daemon starts and shows a desktop notification if the Codex CLI is missing, so package installs do not rely on the user manually checking updater state to understand why Codex App cannot launch cleanly.
- When the Codex CLI is missing and the launcher starts from an interactive terminal, it now prompts before attempting installation instead of requiring the missing-CLI install behavior to be forced implicitly.

## [0.4.1] - 2026-04-19

### Added

- Debian `postinst` maintainer script for `codex-app-updater` so package installs and upgrades can reload user managers and bring the updater service back online.

### Changed

- Native package install and upgrade flows now make a best-effort attempt to start or re-enable `codex-app-updater.service` for active user sessions across Debian, RPM, and pacman packaging paths.
- `codex-app-updater status` now refreshes cached CLI status before printing and surfaces the current CLI error message in plain-text output.
- Native package maintainer hooks now share a single `codex-app-updater-user-service.sh` helper for `systemd --user` reload, start, enable, stop, and disable behavior across Debian, RPM, and pacman packaging paths.
- Packaging hook scripts now use explicit `shellcheck source=...` directives when sourcing the installed user-service helper so static linting can resolve the shared helper path cleanly.

### Fixed

- Restored the final success notification after automatic installs by replaying the `Installed` notification when the updater recovers from an interrupted `Installing` state or daemon restart.
- Deduplicated `Installed` notifications so successful recovery does not spam repeated desktop toasts.
- Hardened Codex CLI version-check caching and error handling so stale cached data does not mask a changed local CLI version or a failed version read.
- `PersistedState::save` now replaces `state.json` atomically with a temporary file and rename, so ad-hoc `codex-app-updater status` refreshes cannot leave partially written updater state behind during concurrent access.

## [0.4.0] - 2026-04-13

### Added

- Automatic Codex CLI installation during launcher preflight when the CLI is missing, exposed through the updater `cli-preflight --allow-install-missing` flow.
- Linux `Open in File Manager` integration in the patched app bundle.
- Launcher-side webview origin validation before Electron starts, with clearer diagnostics when port `5175` serves the wrong content or exits early.
- Expanded smoke coverage for Linux launcher generation and UI patching behavior.

### Changed

- Linux ASAR patching now also adjusts shell behavior, window icon handling, and default opaque window settings on Linux when the user has not explicitly chosen a translucent sidebar preference yet.
- Desktop notifications now resolve icons from packaged, system, and repository locations and send them as file URIs for better desktop-environment compatibility.
- `scripts/install-deps.sh` now owns the `7zz` bootstrap flow, probes pinned upstream tarballs newest-first with `HEAD` checks, and installs to `~/.local/bin` by default unless `SEVENZIP_SYSTEM_INSTALL=1`.
- Updated bundled dependencies and metadata: Electron `40.8.5`, `tokio` `1.51.1`, `windows-sys` `0.61.2`, and `codex-app-updater` `0.4.0`.

### Fixed

- Avoid Linux startup failures caused by stale minified symbol assumptions in the window icon patch (`t.join is not a function`).
- Make updater SHA-256 formatting deterministic so downloaded DMGs produce stable candidate versions and comparisons.
- Prevent `bootstrap_7zz` from warning on unsupported architectures when a working `7zz` or a new enough system `7z` is already available.
- Keep the Linux file manager patch fail-soft when upstream minified bundles drift while still validating that the expected Linux hooks were actually applied.

## [0.3.2] - 2026-04-07

### Fixed

- Fix transparent background flickering on Linux when moving the window or hovering over the sidebar. The upstream Electron app sets `backgroundColor: '#00000000'` (fully transparent) for non-Windows platforms, relying on macOS vibrancy. Linux has no compositor equivalent, causing the desktop to bleed through. The main bundle is now patched to use opaque theme-aware colors (`#000000` dark / `#f9f9f9` light) on Linux.
- Replace transparent startup background in `index.html` with `#1e1e1e` to prevent flash of transparency during app load.

## [0.3.1] - 2026-04-07

### Added

- CLI preflight: before Electron launches, the updater verifies the installed Codex CLI and updates it if a newer npm version is available. Uses a 1-hour cooldown for registry checks and falls back to `npm install -g --prefix ~/.local` if global install fails. Warns instead of blocking app launch on failure.
- Interrupted install recovery: if updater state is left in `Installing` after a crash or restart, the daemon now recovers automatically instead of getting stuck.
- Notification icon resolution chain: bundled, system, repo, then fallback name.
- Makefile targets: `run-app`, `service-enable`, `service-status`.

### Fixed

- `npm install -g` now falls back to `--prefix ~/.local` when global install requires root.

## [0.2.1] - 2026-04-02

### Added

- Native Arch Linux (pacman) package support for updater and install flow.
- Updater builder bundle fix for Arch rebuilds.
- User-local desktop integration (desktop entry, icon, systemd service for non-root installs).

### Fixed

- GPU compositing flickering: added `--disable-gpu-compositing` Electron flag.
- Recoverable 7z warnings handled; added `--fresh` / `--reuse-dmg` flags to installer.
- Graceful patching in `patch-linux-window-ui.js` (warn + skip instead of throw).

## [0.2.0] - 2026-03-27

### Added

- Fedora/RPM packaging support and update manager RPM integration.
- `scripts/install-deps.sh` for automated dependency installation.
- Shared native builders and hardened launcher startup.
- Packaged runtime helper (`codex-packaged-runtime.sh`).
- Failed privileged install no longer auto-retries every reconcile cycle.

### Fixed

- Privilege escalation uses installed binary for self-update.
- Pending install recovery from failed state.
- NVM toolchain preferred for service rebuilds.

## [0.1.0] - 2026-03-20

### Added

- Initial release: automated macOS DMG to Linux Electron app conversion.
- Debian (`.deb`) packaging.
- `codex-app-updater` daemon with systemd user service.
- Upstream DMG detection, local rebuild, and pending install flow.
- Nix flake for NixOS support.
- Wayland and X11 support with GPU error workarounds.
