# Fork Divergences

This reference records intentional contracts in this fork that need explicit
review during upstream syncs. Keep it current when a change adds, removes, or
relocates fork-specific behavior.

Use this document as policy during upstream syncs. Current source files and
templates are authoritative over generated output, but incoming upstream changes
must be reconciled against the contracts below before a sync is pushed.

## Sync Review Rule

For each upstream sync:

1. Read this inventory before editing or resolving conflicts.
2. Compare incoming changes against every divergence area below.
3. Preserve local names, paths, versioning, updater boundaries, package shape,
   and security gates unless the PR intentionally changes this policy.
4. Put uncertain conflicts in the PR body for maintainer triage.
5. Run the local build gate before pushing when generated-app, package, updater
   rebuild, or bundled runtime behavior is touched.

The layout rules for this fork follow, in order, the XDG Base Directory
Specification, the Filesystem Hierarchy Standard, and common distro conventions
for modern Electron-style app bundles.

## Divergence Inventory

### 1. Fork Identity: `codex-app` And `codex-app-updater`

**Intent:** The Linux app, package, launcher, desktop entry, icon name, app
state, and package metadata use `codex-app`. The updater crate, updater binary,
user service, XDG config, state, cache, and logs use `codex-app-updater`.
Compatibility references to `codex-desktop` or `codex-update-manager` are
allowed only for package transition metadata or legacy cleanup.

**Why it matters:** These names are user-visible package and runtime contracts.
Adopting upstream names during a sync breaks upgrade paths, service state,
desktop integration, docs, and user commands.

**Current paths:** `install.sh`, `launcher/start.sh.template`,
`packaging/linux/`, `scripts/build-deb.sh`, `scripts/build-rpm.sh`,
`scripts/build-pacman.sh`, `scripts/lib/package-common.sh`, `updater/`,
`contrib/user-local-install/`, `README.md`, `CHANGELOG.md`,
`docs/maintainers/package-runtime-maintenance.md`.

**Refs:** `ad53654`, `a4a10e9`, `c9784fd`, `0a872de`, `96c65d4`,
`9482d34`, `e640798`, `cf41366`, `fd0d838`, `bb5c0f6`; fork PRs #3, #10, #11.

**Preservation checks:** Search user-facing docs, package metadata, desktop
entries, services, updater paths, and launcher commands for upstream names.
Keep legacy names only where comments or metadata explain the compatibility
purpose.

### 2. Linux Filesystem Layout And Package Payload Contract

**Intent:** Native packages install the generated Electron app under
`/opt/codex-app`, package-private support under `/usr/lib/codex-app`,
commands under `/usr/bin`, shared desktop assets under `/usr/share`, and
mutable user files under XDG base directories.

**Why it matters:** This layout matches distro expectations for package-managed
Electron app bundles and keeps mutable user state out of system package roots.

**Current paths:** `packaging/linux/PKGBUILD.template`,
`packaging/linux/control`, `packaging/linux/codex-app.spec`,
`packaging/linux/codex-app.install`, Debian/RPM maintainer scripts,
`packaging/linux/codex-packaged-runtime.sh`, `scripts/lib/package-common.sh`,
`launcher/start.sh.template`, `updater/src/config.rs`, `updater/src/app.rs`,
`updater/src/builder.rs`, `contrib/user-local-install/`.

**Refs:** `ad53654`, `a4a10e9`, `176b957`, `5ce96c9`, `96c65d4`,
`9482d34`, `e640798`, `52b10ff`, `cf41366`; fork PRs #7 and #10.

**Preservation checks:** Inspect package file lists and source templates for
`/opt/codex-app`, `/usr/lib/codex-app`, `/usr/bin/codex-app`,
`/usr/bin/codex-app-updater`, and XDG paths. Do not adopt `~/.local/opt` or
upstream package roots.

### 3. Package Versioning From The OpenAI DMG Bundle

**Intent:** Native package versions come from the OpenAI app bundle's
`CFBundleShortVersionString`, written to `codex-app/codex-app-version.env`
during app generation. Timestamp or commit-hash package versions are explicit
test overrides only.

**Why it matters:** Package upgrades, updater comparisons, release notes, and
user expectations should track the upstream app version, not local build time.

**Current paths:** `install.sh`, `scripts/lib/dmg.sh`,
`scripts/lib/package-common.sh`, package builders, `updater/src/app.rs`,
`updater/src/builder.rs`, `updater/src/package_version.rs`,
`updater/src/upstream.rs`, `README.md`, `docs/usage/build-and-run.md`,
`tests/scripts_smoke.sh`.

**Refs:** `c9784fd`, `3bc215a`, `840a413`, `bd47812`, `9dc4f1a`,
`e8208e0`, `96c65d4`, `bb5c0f6`; fork PRs #3, #10, #11.

**Preservation checks:** Run `make help` and package docs checks to ensure
plain `make deb`, `make rpm`, and `make pacman` are the normal path. Keep
`PACKAGE_VERSION=...` documented only as a deliberate override.

### 4. Native Package Builders And Pacman Ownership

**Intent:** Debian, RPM, and pacman builders share staging helpers, validate
generated app payload symlinks, normalize payload modes, and avoid preserving
local build ownership into pacman archives.

**Why it matters:** Native packages must install with package-manager-owned
system paths, predictable modes, and aligned payloads across formats.

**Current paths:** `scripts/build-deb.sh`, `scripts/build-rpm.sh`,
`scripts/build-pacman.sh`, `scripts/lib/package-common.sh`,
`packaging/linux/control`, `packaging/linux/codex-app.spec`,
`packaging/linux/PKGBUILD.template`, `packaging/linux/codex-app.install`,
`tests/scripts_smoke.sh`.

**Refs:** `ad53654`, `a4a10e9`, `176b957`, `83e910d`, `2e33bc7`,
`5ce96c9`, `96c65d4`, `9482d34`, `52b10ff`, `cf41366`; fork PRs #4, #7, #10.

**Preservation checks:** Build the affected package format and inspect metadata
plus the first file-listing page. For pacman, check that package ownership is
not inherited from the local build user.

### 5. Updater Privilege Boundary And Install Hardening

**Intent:** `codex-app-updater` remains unprivileged until the final native
package install. Privileged work is limited to `install-deb`, `install-rpm`,
and `install-pacman`, which validate package paths, stage private copies, check
package identity metadata, and then invoke the package manager.

**Why it matters:** The updater handles mutable network inputs and local build
work. Privilege must stay isolated to the smallest install surface.

**Current paths:** `updater/src/install.rs`, `updater/src/app.rs`,
`updater/src/config.rs`, `updater/src/builder.rs`,
`packaging/linux/com.github.nisavid.codex-app.update.policy`,
`packaging/linux/codex-app-updater.service`, maintainer scripts,
`docs/maintainers/security-backlog.md`, `docs/maintainers/threat-model.md`.

**Refs:** `9f1bc89`, `93f10af`, `1f58d49`, `1095916`, `e200937`,
`144abfb`, `83e910d`, `2e33bc7`, `c2483c5`, `680dba5`, `25f2764`,
`29b60bd`, `952c3c3`, `96767e7`, `35db113`, `96c65d4`, `cf41366`,
`df1f12c`; fork PRs #4, #9, #10.

**Preservation checks:** Run updater install tests or targeted review for
state/install changes. Route new trust-boundary work through the security
backlog and `@codex-security` workflow.

### 6. Updater State, Config Overlay, And Failure Recovery

**Intent:** Updater config and state use XDG paths, user config is a partial
overlay, legacy state keys remain readable, failed or dismissed installs do not
reprompt in a loop, interrupted installs recover, and production builder
redirection requires explicit `developer_mode = true`.

**Why it matters:** The updater runs continuously and needs stable persisted
state across package upgrades, crashes, and user configuration changes.

**Current paths:** `updater/src/app.rs`, `updater/src/config.rs`,
`updater/src/upstream.rs`, `updater/src/builder.rs`, `updater/src/install.rs`,
`updater/src/package_version.rs`, `updater/src/codex_cli.rs`,
`.github/workflows/updater.yml`, `docs/usage/troubleshooting.md`.

**Refs:** `3b67a39`, `020a063`, `f587a02`, `144abfb`, `680dba5`,
`035c1dd`, `4c14a65`, `29b60bd`, `952c3c3`, `35db113`, `96c65d4`,
`cf41366`, `3054a78`; fork PRs #4, #7, #9, #10.

**Preservation checks:** Run full updater tests for state, install, CLI
preflight, liveness, or daemon control-flow changes.

### 7. Codex CLI Discovery And Preflight

**Intent:** CLI discovery checks explicit CLI options, `CODEX_CLI_PATH`,
updater config, persisted updater state, launch `PATH`, and known user-local
package-manager paths. The launcher gives updater preflight a fast path before
direct fallback, prompts for missing CLI installation where interactive, and
exports `CODEX_CLI_PATH` before Electron starts.

**Why it matters:** The app needs a reliable Codex CLI path without blocking
Electron startup on registry or install work that can run later.

**Current paths:** `launcher/start.sh.template`, `install.sh`,
`updater/src/codex_cli.rs`, `updater/src/config.rs`, `updater/src/app.rs`,
`updater/src/main.rs`, `updater/src/state.rs`,
`docs/usage/troubleshooting.md`, `.github/workflows/updater.yml`.

**Refs:** `3b67a39`, `6056625`, `020a063`, `69a0af4`, `4c14a65`,
`8955a10`, `96c65d4`, `52b10ff`, `cf41366`; fork PRs #7, #9, #10.

**Preservation checks:** Keep synchronous path resolution separate from
background npm registry/update checks in docs and tests. Invalid configured
paths should fail loudly; stale persisted paths should not block fallback.

### 8. Generated Launcher And Packaged Runtime Behavior

**Intent:** Checkout launches remain generic. Native packages load
package-only behavior only when the packaged runtime helper exists. The helper
imports desktop/session display variables, does not import `PATH`, disables the
legacy update-manager service name when present, starts/enables
`codex-app-updater.service` without restarting an active service, and triggers
launch-time update checks only after the launcher records Electron's PID.

**Why it matters:** Package-specific service orchestration must not leak into
checkout builds or race pending updater install state.

**Current paths:** `launcher/start.sh.template`, `install.sh`,
`packaging/linux/codex-packaged-runtime.sh`,
`packaging/linux/codex-app-updater.service`,
`packaging/linux/codex-app-updater-user-service.sh`,
`scripts/lib/package-common.sh`, `tests/scripts_smoke.sh`.

**Refs:** `a4a10e9`, `f587a02`, `1f58d49`, `e200937`, `956ceac`,
`094caef`, `8252165`, `96c65d4`, `9482d34`, `52b10ff`, `cf41366`,
`83ef0fc`; fork PRs #4, #9, #10.

**Preservation checks:** Change package-only launcher behavior in
`packaging/linux/codex-packaged-runtime.sh`, then inspect regenerated
`codex-app/start.sh`.

### 9. ASAR And Linux UI Patch Behavior

**Intent:** Linux ASAR patches are fail-soft for volatile upstream bundle
shapes, preserve Linux window defaults and launch controls, default Chromium
sandboxing on, use `CODEX_APP_LAUNCH_ACTION_SOCKET`, sanitize generated
keybind patch literals, and default Linux `opaqueWindows` only when no user
preference exists.

**Why it matters:** Upstream minified bundle shapes change often. Linux
behavior should degrade with actionable warnings instead of breaking app
generation unless a required invariant fails.

**Current paths:** `scripts/patch-linux-window-ui.js`,
`scripts/patch-linux-window-ui.test.js`, `scripts/lib/asar-patch.sh`,
`install.sh`, `launcher/start.sh.template`, `tests/scripts_smoke.sh`,
`docs/usage/troubleshooting.md`.

**Refs:** `2f18ea2`, `cbaf131`, `cf0db9d`, `e365289`, `96c65d4`,
`52b10ff`, `cf41366`, `6a3d879`; fork PRs #4 and #10.

**Preservation checks:** Run the Node patch tests and shell smoke tests when
ASAR patchers or launch flags change.

### 10. Webview Server Lifecycle

**Intent:** The launcher binds the webview server to loopback, verifies
startup assets before Electron launch, records the webview PID under XDG state,
reuses verified warm-start servers, and avoids deleting unrelated live app
markers.

**Why it matters:** Codex expects webview assets at a local origin, while Linux
launches must avoid LAN exposure, stale servers, and PID ownership races.

**Current paths:** `launcher/start.sh.template`, `scripts/lib/webview-install.sh`,
`install.sh`, `docs/webview-server-evaluation.md`,
`docs/usage/troubleshooting.md`, `tests/scripts_smoke.sh`.

**Refs:** `f587a02`, `956ceac`, `7512063`, `8252165`, `e365289`,
`52b10ff`, `83ef0fc`; fork PRs #4, #8, #9, #10.

**Preservation checks:** Use `docs/webview-server-evaluation.md` before
changing the local server model, port behavior, or warm-start adoption.

### 11. Linux Computer Use Packaging

**Intent:** Linux Computer Use support is packaged with the fork's app,
including the Rust MCP backend and bundled plugin resources. Local compatibility
adjustments include an error when a requested app is not found and a plugin
manifest logo path that points at a present SVG asset.

**Why it matters:** The package can stage local Computer Use support, but the
Codex UI remains gated by OpenAI account-side rollout and host accessibility or
input prerequisites.

**Current paths:** `computer-use-linux/`,
`plugins/openai-bundled/plugins/computer-use/`,
`scripts/lib/bundled-plugins.sh`, `scripts/lib/package-common.sh`,
`launcher/start.sh.template`, `README.md`, `CHANGELOG.md`.

**Refs:** `e365289`; fork PR #10.

**Preservation checks:** Keep package staging and README wording clear that
local installation does not bypass OpenAI feature flags.

### 12. Release, Security, And Supply-Chain Verification

**Intent:** Release and security workflow includes trusted DMG hash input,
generated app and ASAR inspection, package metadata checks, checksums, optional
detached checksum signing, public key export, macOS Apple DMG verification,
reviewed hash-refresh PRs, and safer DMG URL/download handling.

**Why it matters:** This fork rebuilds a package from a mutable upstream DMG
URL. Release and updater work must leave reviewable evidence and avoid
presenting unverified artifacts as trusted.

**Current paths:** `.github/workflows/update-codex-hash.yml`,
`.github/workflows/verify-apple-dmg.yml`, `.github/workflows/ci.yml`,
`.github/workflows/updater.yml`, `Makefile`, `flake.nix`,
`scripts/release-gate.sh`, `scripts/verify-apple-dmg.sh`,
`scripts/inspect-electron-security.js`, `scripts/lib/dmg.sh`,
`updater/src/upstream.rs`, `updater/src/app.rs`,
`docs/maintainers/security-backlog.md`, `docs/maintainers/threat-model.md`.

**Refs:** `7caa964`, `f587a02`, `d77d15a`, `d13973d`, `cd60b2a`,
`7789b05`, `773d9d8`, `cbaf131`, `1694d83`, `5b50b49`, `a79c572`,
`680dba5`, `25f2764`, `089dc4a`, `1861e17`, `74e11f1`, `96c65d4`,
`3054a78`, `52b10ff`; fork PRs #4, #5, #6, #10.

**Preservation checks:** `make help` must show `apple-dmg-verify` and
`release-gate`. Security backlog items that change trust boundaries should use
the `@codex-security` workflow before review-ready handoff.

### 13. User-Local Install Experiment

**Intent:** The experimental unprivileged install path uses `codex-app`
commands, service/timer names, desktop entry, and XDG user data paths. It stays
aligned with fork path triage but remains separate from native package layout.

**Why it matters:** The rootless experiment should not reintroduce upstream
names or non-XDG paths while testing a different install model.

**Current paths:** `contrib/user-local-install/README.md`,
`contrib/user-local-install/install-user-local.sh`,
`contrib/user-local-install/files/.config/systemd/user/`,
`contrib/user-local-install/files/.local/bin/`,
`contrib/user-local-install/files/.local/share/applications/`,
`contrib/user-local-install/files/share/common.sh`.

**Refs:** `b4ccd9a`, `a4a10e9`, `cf0db9d`, `e365289`, `9482d34`; fork PR #10.

**Preservation checks:** Keep the payload under
`${XDG_DATA_HOME:-~/.local/share}/codex-app`; do not use `~/.local/opt`.

### 14. Maintainer Policy, Docs, And Agent Workflow

**Intent:** Durable maintenance behavior lives in always-loaded policy,
maintainer docs, and triggered skills: branch before work, Conventional
Commits, local build gates before pushing app/package/updater/runtime changes,
24-hour DMG freshness checks, source-of-truth boundaries, upstream-sync
contracts, and security review workflows. The README stays user-facing.

**Why it matters:** This fork is intentionally divergent from its direct
upstream. Future agents need durable, discoverable policy without turning the
README or `AGENTS.md` into large maintenance manuals.

**Current paths:** `AGENTS.md`,
`.agents/skills/maintaining-codex-app-package/SKILL.md`, `docs/README.md`,
`docs/backlog.md`, `docs/maintainers/package-runtime-maintenance.md`,
`docs/maintainers/security-backlog.md`, `docs/maintainers/threat-model.md`,
`docs/policies/agentic-maintenance.md`, `docs/usage/`, `README.md`,
`CHANGELOG.md`.

**Refs:** `ad53654`, `b4ccd9a`, `0a872de`, `d3ddf25`, `1861e17`,
`7cbc44b`, `7be5ca3`, `16093f5`, `74e11f1`, `96c65d4`, `e640798`,
`fd0d838`, `bb5c0f6`; fork PRs #3, #4, #6, #7, #10, #11.

**Preservation checks:** Keep `AGENTS.md` short and route details to maintainer
docs or repo-local skills. Check README audience, clone URLs, maintainer-only
material, and always-loaded policy length before merging sync PRs.

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
