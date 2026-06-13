# Fork Divergences

This reference records the intentional differences between this fork and the
last synced ref from the Linux-port upstream. In this document, `upstream` means
that remote unless a sentence names another surface. Use this inventory during
upstream syncs to preserve local contracts and keep divergence claims grounded
in the synced baseline. Treat these differences as a finishing layer: upstream
owns the primary Linux app conversion and much of the runtime support, while
this fork preserves local names, paths, updater policy, hardening, security
review, packaging polish, and maintainer policy.

## Upstream Terminology

Use the same terms as `AGENTS.md`:

- `Linux-port upstream`: `ilysenko/codex-desktop-linux`, the git remote named
  `upstream`, and sync work that imports that repository's Linux conversion
  changes.
- `Official OpenAI Codex DMG`: the OpenAI-distributed macOS app artifact used
  as app-generation input.
- `Official OpenAI app bundle`: the `Codex.app` bundle extracted from the DMG
  and patched for Linux.
- `OpenAI-hosted services`: account, rollout, entitlement, remote-control, and
  other service-side behavior outside this fork's local packaging path.

Use the specific term when a reader could confuse the Linux-port upstream with
the official OpenAI app, DMG, app bundle, or hosted services. Once the surface
is clear, concise terms such as `upstream`, `DMG`, or `app bundle` are fine.

The current comparison baseline is upstream commit
`1d2bd267ef9a41ae6be5a604150b5c4ef638d984` (2026-06-12). Claims below describe
the current tree's diff against that baseline, with current source files taking
precedence over generated output.

## Sync Review Rule

For each upstream sync:

1. Read this inventory before editing or resolving conflicts.
2. Compare incoming changes against every divergence area below.
3. Preserve local names, paths, versioning, updater boundaries, package shape,
   and security gates unless the PR intentionally changes this policy.
4. Describe each divergence as the current local finishing-layer delta against
   the synced upstream baseline: naming, layout, hardening, packaging,
   compatibility, security review, or documentation.
5. Escalate uncertain conflicts to the operator when the session allows. If
   escalation is unavailable or the operator requested an uninterrupted run,
   record a durable, discoverable follow-up where the escalation would have
   happened, and link it from the sync ledger.
6. Run the local build gate before pushing when generated-app, package, updater
   rebuild, or bundled runtime behavior is touched. The minimum gate is
   `make build-app` or `./install.sh` after the DMG freshness check; package
   payload changes also need the relevant package builder, and release workflow
   changes also need the relevant release-gate command.

The layout rules for this fork follow, in order, the XDG Base Directory
Specification, the Filesystem Hierarchy Standard, and common distro conventions
for modern Electron-style app bundles.

## Current Local Rename And Compatibility Map

Use this map during upstream syncs. If upstream edits an old path or token,
reconcile the change into the current local path or token before deleting the
old target from the merge result.

| Old target or token | Current local target or token | Source and sync relevance |
| --- | --- | --- |
| `.github/workflows/upstream-build-app.yml` | `.github/workflows/official-dmg-build-app.yml` | Exists in `upstream/main`; port incoming workflow edits here. |
| `updater/src/upstream.rs` | `updater/src/dmg_source.rs` | Exists in `upstream/main`; port incoming updater source edits here. |
| patch `ciPolicy: "required-upstream"` | `ciPolicy: "required-official-dmg"`; the old value is accepted only as a legacy alias | Exists in `upstream/main`; port incoming required-patch policy edits to the current token unless intentionally preserving compatibility aliases. |
| patch-report profile `upstream-build` | `official-dmg-build`; the old profile is accepted only as a legacy alias | Exists in `upstream/main`; port incoming validation-profile edits to the current profile name. |
| CI job or local CI target `upstream` for official DMG validation | `official-dmg`; the old target is accepted only as a legacy alias | Exists in `upstream/main`; port incoming official DMG validation job/target changes to the current name. |
| `UPSTREAM_DMG_URL`, `UPSTREAM_DMG_PATH`, `UPSTREAM_DMG_CACHE_HIT` | `OFFICIAL_DMG_URL`, `OFFICIAL_DMG_PATH`, `OFFICIAL_DMG_CACHE_HIT`; old variables are legacy aliases | Exists in `upstream/main`; port incoming official DMG environment changes to the current variables and preserve legacy fallbacks only for compatibility. |
| Port integration hook `CODEX_UPSTREAM_APP_DIR` | `CODEX_OFFICIAL_APP_DIR`; the old variable is a legacy alias | Exists in `upstream/main`; port incoming stage-hook environment changes to the current variable and keep the legacy alias only for existing hooks. |
| Make target `inspect-upstream` | `inspect-dmg`; the old target is a legacy alias | Exists in `upstream/main`; port incoming inspect-target behavior to `inspect-dmg` and keep the old target as an alias only while useful. |
| `packaging/appimage/codex-desktop.desktop` | `packaging/appimage/codex-app.desktop` | Exists in `upstream/main`; port incoming AppImage desktop-entry edits to the current local AppImage desktop entry. |
| `packaging/linux/codex-desktop.spec`, `packaging/linux/codex-desktop.install`, `packaging/linux/codex-desktop.desktop`, and `packaging/linux/codex-desktop-entry-doctor.sh` | `packaging/linux/codex-app.spec`, `packaging/linux/codex-app.install`, `packaging/linux/codex-app.desktop`, and `packaging/linux/codex-app-desktop-entry-doctor.sh` | Exists in `upstream/main`; port incoming native package identity and desktop-integration edits to the current local package files. |
| `packaging/linux/codex-update-manager.service`, `packaging/linux/codex-update-manager-user-service.sh`, `packaging/linux/codex-update-manager.postinst`, `packaging/linux/codex-update-manager.postrm`, and `packaging/linux/codex-update-manager.prerm` | `packaging/linux/codex-app-updater.service`, `packaging/linux/codex-app-updater-user-service.sh`, `packaging/linux/codex-app-updater.postinst`, `packaging/linux/codex-app-updater.postrm`, and `packaging/linux/codex-app-updater.prerm` | Exists in `upstream/main`; port incoming updater service and maintainer-script edits under the local updater identity. |
| `packaging/linux/com.github.ilysenko.codex-desktop-linux.update.policy` | `packaging/linux/com.github.nisavid.codex-app.update.policy` | Exists in `upstream/main`; port incoming privileged install policy edits to the local policy file and preserve the local action identifiers. |
| `contrib/user-local-install/files/.config/systemd/user/codex-desktop-update.service`, `contrib/user-local-install/files/.config/systemd/user/codex-desktop-update.timer`, `contrib/user-local-install/files/.local/bin/codex-desktop*`, `contrib/user-local-install/files/.local/share/applications/codex-desktop.desktop`, and `contrib/user-local-install/files/.local/lib/codex-desktop-linux/common.sh` | `contrib/user-local-install/files/.config/systemd/user/codex-app-update.service`, `contrib/user-local-install/files/.config/systemd/user/codex-app-update.timer`, `contrib/user-local-install/files/.local/bin/codex-app*`, `contrib/user-local-install/files/.local/share/applications/codex-app.desktop`, and `contrib/user-local-install/files/share/common.sh` | Exists in `upstream/main`; port incoming user-local install experiment edits to the current local names and layout. |
| `linux-features/` | `port-integrations/`; the old root is accepted only as a legacy override target | Exists in the Linux-port upstream's old registry naming; port incoming registry edits to `port-integrations/`. |
| `linux-features/*/feature.json` | `port-integrations/*/integration.json`; old manifests are accepted only for legacy roots | Exists in the Linux-port upstream's old registry naming; port incoming manifest edits to the current manifest path. |
| `linux-features/features.example.json` and `linux-features/features.json` | `port-integrations/integrations.example.json` and `port-integrations/integrations.json`; old names are compatibility fallbacks | Exists in the Linux-port upstream's old registry naming; port incoming config-shape changes to the current config names. |
| `scripts/lib/linux-features.js` and `scripts/lib/linux-features.sh` | `scripts/lib/port-integrations.js` and `scripts/lib/port-integrations.sh` | Exists in the Linux-port upstream's old registry naming; port incoming helper changes to the current helper names. |
| `CODEX_LINUX_FEATURES_ROOT`, `CODEX_LINUX_FEATURES_CONFIG`, `CODEX_LINUX_FEATURES`, `CODEX_LINUX_DISABLE_FEATURES` | `CODEX_PORT_INTEGRATIONS_ROOT`, `CODEX_PORT_INTEGRATIONS_CONFIG`, `CODEX_PORT_INTEGRATIONS`, `CODEX_DISABLE_PORT_INTEGRATIONS`; old variables are legacy aliases | Exists in the Linux-port upstream's old registry naming; port incoming environment handling to the current variables and preserve aliases only for compatibility. |
| `CODEX_BOOTSTRAP_CLEANUP_FEATURES` | `CODEX_BOOTSTRAP_CLEANUP_INTEGRATIONS`; the old variable is a legacy alias | Exists in earlier local setup helper behavior; use the current variable in docs and tests. |

## Divergence Inventory

### 1. Local Product And Package Identity

**Fork delta:** Upstream uses the `codex-desktop` app/package
identity and the `codex-update-manager` updater identity. This fork
intentionally exposes the app, packages, launcher, desktop entry, icon, app
state, and package metadata as `codex-app`. It exposes the updater crate,
binary, service, config, state, cache, and logs as `codex-app-updater`.

**Upstream baseline:** The underlying Linux app conversion and update manager
model come from upstream. The fork-specific contract is the local identity and
compatibility handling around that inherited model.

**Why it matters:** These names are user-visible package and runtime contracts.
Adopting upstream names during a sync breaks upgrade paths, service state,
desktop integration, docs, and user commands.

**Current paths:** `Cargo.toml`, `updater/Cargo.toml`, `Makefile`,
`install.sh`, `launcher/start.sh.template`, `packaging/linux/`,
`scripts/build-deb.sh`, `scripts/build-rpm.sh`, `scripts/build-pacman.sh`,
`scripts/lib/package-common.sh`, `updater/`, `contrib/user-local-install/`,
`README.md`, `CHANGELOG.md`,
`docs/maintainers/package-runtime-maintenance.md`.

**Preservation checks:** Search user-facing docs, package metadata, desktop
entries, services, updater paths, and launcher commands for upstream names.
Keep `codex-desktop` and `codex-update-manager` only where comments or package
metadata explain legacy transition behavior.

### 2. Linux Filesystem Layout And Package Payload Contract

**Fork delta:** Native packages keep the generated app bundle under
`/opt/codex-app`, package-private support under `/usr/lib/codex-app`, launchers
under `/usr/bin`, desktop assets under `/usr/share`, and mutable user files
under XDG base directories. The update-builder bundle is deliberately under
`/usr/lib/codex-app/update-builder`, not inside the generated app bundle.

**Upstream baseline:** Upstream already has package builders and an
update-builder payload. This fork changes the installed names and payload
placement, and keeps those choices aligned with XDG/FHS criteria.

**Why it matters:** This layout matches distro expectations for package-managed
Electron app bundles and keeps mutable user state out of system package roots.

**Current paths:** `packaging/linux/PKGBUILD.template`,
`packaging/linux/control`, `packaging/linux/codex-app.spec`,
`packaging/linux/codex-app.install`, Debian/RPM maintainer scripts,
`packaging/linux/codex-packaged-runtime.sh`, `scripts/lib/package-common.sh`,
`launcher/start.sh.template`, `updater/src/config.rs`, `updater/src/app.rs`,
`updater/src/builder.rs`, `contrib/user-local-install/`.

**Preservation checks:** Inspect package file lists and source templates for
`/opt/codex-app`, `/usr/lib/codex-app`, `/usr/bin/codex-app`,
`/usr/bin/codex-app-updater`, and XDG paths. Do not adopt `~/.local/opt`,
`/opt/codex-desktop`, or upstream support-bundle paths during a sync.

### 3. Package Versioning From The OpenAI DMG Bundle

**Fork delta:** Package versions default to the OpenAI app bundle's
`CFBundleShortVersionString`, written to `codex-app/codex-app-version.env`
during app generation. Timestamp or commit-hash package versions are explicit
test overrides only.

**Upstream baseline:** Upstream already derives update
candidates from official OpenAI Codex DMG metadata. This fork changes native
package versioning and updater comparison helpers so package upgrades track the
DMG-contained app version.

**Why it matters:** Package upgrades, updater comparisons, release notes, and
user expectations should track the official app version, not local build time.

**Current paths:** `install.sh`, `scripts/lib/dmg.sh`,
`scripts/lib/package-common.sh`, package builders, `updater/src/app.rs`,
`updater/src/builder.rs`, `updater/src/package_version.rs`,
`updater/src/dmg_source.rs`, `README.md`, `docs/usage/build-and-run.md`,
`tests/scripts_smoke.sh`.

**Preservation checks:** Run `make help` and package docs checks to ensure
plain `make deb`, `make rpm`, and `make pacman` are the normal path. Keep
`PACKAGE_VERSION=...` documented only as a deliberate override.

### 4. Package Builder Hardening

**Fork delta:** The Debian, RPM, pacman, and AppImage builders keep local
names, replacement metadata, package output names, and staged payloads aligned
with their intended package surfaces. The shared staging helper validates
native package inputs, rejects unsafe app payload symlinks, normalizes payload
modes, avoids preserving local build ownership into pacman packages, and prints
package metadata/content inspection where tools support it. AppImage stays a
manual-update artifact without updater service, polkit, or update-builder
payload.

**Upstream baseline:** Upstream already builds native packages
and now carries a local AppImage target. This fork adds hardening, local
identity, and payload consistency constraints.

**Why it matters:** Native packages must install with package-manager-owned
system paths, predictable modes, and aligned payloads across formats.

**Current paths:** `scripts/build-deb.sh`, `scripts/build-rpm.sh`,
`scripts/build-pacman.sh`, `scripts/build-appimage.sh`,
`scripts/lib/package-common.sh`, `packaging/linux/control`,
`packaging/linux/codex-app.spec`, `packaging/linux/PKGBUILD.template`,
`packaging/linux/codex-app.install`, `packaging/appimage/`,
`tests/scripts_smoke.sh`.

**Preservation checks:** Build the affected package format and inspect metadata
plus the first file-listing page. For pacman, check that package ownership is
not inherited from the local build user. For AppImage, check that updater-only
service and polkit files are absent.

### 5. Updater Privilege Boundary And Install Hardening

**Fork delta:** `codex-app-updater` remains unprivileged until the final native
package install. Privileged work runs only through `install-deb`, `install-rpm`,
and `install-pacman`, which validate package paths and identity metadata,
stage private copies, and then invoke the package manager through `pkexec`.

**Upstream baseline:** Upstream already has a user-level update
manager and privileged package install path. This fork tightens the boundary and
renames the service, policy, and package identities.

**Why it matters:** The updater handles mutable network inputs and local build
work. Privilege must stay isolated to the smallest install surface.

**Current paths:** `updater/src/install.rs`, `updater/src/app.rs`,
`updater/src/config.rs`, `updater/src/builder.rs`,
`packaging/linux/com.github.nisavid.codex-app.update.policy`,
`packaging/linux/codex-app-updater.service`, maintainer scripts,
`docs/maintainers/security-backlog.md`, `docs/maintainers/threat-model.md`.

**Preservation checks:** Run updater install tests or targeted review for
state/install changes. Route new trust-boundary work through the security
backlog and `@codex-security` workflow.

### 6. Updater State, Config Overlay, And Failure Recovery

**Fork delta:** Updater config and state use `codex-app-updater` XDG paths,
user config is a partial overlay, explicit `cli_path` is supported, failed or
dismissed installs avoid prompt loops, interrupted installs recover, and
production builder redirection requires `developer_mode = true`.

**Upstream baseline:** Upstream already has persisted updater state and a
daemon. This fork changes the local names, persisted config surface, recovery
rules, and developer-mode guardrails.

**Why it matters:** The updater runs continuously and needs stable persisted
state across package upgrades, crashes, and user configuration changes.

**Current paths:** `updater/src/app.rs`, `updater/src/config.rs`,
`updater/src/dmg_source.rs`, `updater/src/builder.rs`, `updater/src/install.rs`,
`updater/src/package_version.rs`, `updater/src/codex_cli.rs`,
`.github/workflows/updater.yml`, `docs/usage/troubleshooting.md`.

**Preservation checks:** Run full updater tests for state, install, CLI
preflight, liveness, or daemon control-flow changes.

### 7. Codex CLI Discovery And Preflight

**Fork delta:** CLI discovery uses explicit CLI options, `CODEX_CLI_PATH`,
updater config, persisted updater state, launch `PATH`, and known user-local
package-manager paths. The launcher passes `--cli-path` only when a path is
known, gives updater preflight a fast path before direct fallback, prompts for
missing CLI installation where interactive, and exports `CODEX_CLI_PATH`
before Electron starts.

**Upstream baseline:** Upstream already has launcher/updater CLI
preflight. This fork refines discovery precedence, config integration, and
best-effort behavior under the `codex-app-updater` identity.

**Why it matters:** The app needs a reliable Codex CLI path without blocking
Electron startup on registry or install work that can run later.

**Current paths:** `launcher/start.sh.template`, `install.sh`,
`updater/src/codex_cli.rs`, `updater/src/config.rs`, `updater/src/app.rs`,
`updater/src/main.rs`, `updater/src/state.rs`,
`docs/usage/troubleshooting.md`, `.github/workflows/updater.yml`.

**Preservation checks:** Keep synchronous path resolution separate from
background npm registry/update checks in docs and tests. Invalid configured
paths should fail loudly; stale persisted paths should not block fallback.

### 8. Generated Launcher And Packaged Runtime Behavior

**Fork delta:** Checkout launches stay generic. Native packages load
package-only behavior only when the packaged runtime helper exists. The helper
lives under `/usr/lib/codex-app`, imports desktop/session display variables
without importing `PATH`, disables the legacy upstream service name when
present, starts/enables `codex-app-updater.service` without restarting an active
service, and triggers launch-time update checks after Electron PID recording.

**Upstream baseline:** Upstream provides the launcher template and packaged
runtime pattern. This fork changes the package-only helper location, service
names, environment import policy, and lifecycle details.

**Why it matters:** Package-specific service orchestration must not leak into
checkout builds or race pending updater install state.

**Current paths:** `launcher/start.sh.template`, `install.sh`,
`packaging/linux/codex-packaged-runtime.sh`,
`packaging/linux/codex-app-updater.service`,
`packaging/linux/codex-app-updater-user-service.sh`,
`scripts/lib/package-common.sh`, `tests/scripts_smoke.sh`.

**Preservation checks:** Change package-only launcher behavior in
`packaging/linux/codex-packaged-runtime.sh`, then inspect regenerated
`codex-app/start.sh`.

### 9. ASAR, Port Integration, And Linux UI Patch Behavior

**Fork delta:** ASAR patches remain fail-soft for volatile official app bundle
shapes. The current fork delta includes local identity updates, sanitized
generated keybind literals, `CODEX_APP_LAUNCH_ACTION_SOCKET`, Linux window
default refinements, opt-in multi-instance launch support, default-enabled
Electron sandboxing with an explicit compatibility opt-out, and default-enabled
Open target discovery through the port integration registry. It also keeps the Linux
Computer Use plugin manifest gate default-on while keeping Computer Use UI
patches opt-in through
`CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1` or the persisted
`codex-linux-computer-use-ui-enabled` setting. Remote-control UI and mobile
remote-control host patches are default-enabled port integrations and keep private
device-key material under `${XDG_CONFIG_HOME:-~/.config}/codex-app`.

**Upstream baseline:** Upstream already carries Linux ASAR patching. This fork
maintains local patch safety and selected Linux behavior changes on top of that
patching system.

**Naming policy:** Durable docs call configurable modules port integrations.
The source path is `port-integrations/`, manifests are `integration.json`,
configs are `integrations.json` or `port-integrations.json`, and environment
variables use `CODEX_PORT_INTEGRATIONS_*`. If upstream changes a module under
the old `linux-features/` naming scheme, port the change to the current local
path and preserve the docs terminology.

**Why it matters:** Official app minified bundle shapes change often. Linux
behavior should degrade with actionable warnings instead of breaking app
generation unless a required invariant fails.

**Current paths:** `scripts/patch-linux-window-ui.js`,
`scripts/patch-linux-window-ui.test.js`, `scripts/lib/asar-patch.sh`,
`scripts/lib/port-integrations.js`, `port-integrations/open-target-discovery/`,
`port-integrations/remote-control-ui/`, `port-integrations/remote-mobile-control/`,
`port-integrations/integrations.example.json`, `install.sh`,
`launcher/start.sh.template`, `tests/scripts_smoke.sh`,
`docs/usage/troubleshooting.md`.

**Preservation checks:** Run the Node patch tests and shell smoke tests when
ASAR patchers or launch flags change.

### 10. Webview Server Lifecycle

**Fork delta:** The launcher keeps webview server state under the local app
identity and XDG state paths, preserves live app markers during warm-start or
second-instance handoff, and keeps origin validation tied to loopback startup
assets plus `.codex-linux/webview-integrity.sha256` before Electron launch.

**Upstream baseline:** Upstream already has the local webview server model and
much of the launcher lifecycle. This fork preserves and renames that behavior
while maintaining the local XDG/path contract.

**Why it matters:** Codex expects webview assets at a local origin, while Linux
launches must avoid LAN exposure, stale servers, and PID ownership races.

**Current paths:** `launcher/start.sh.template`,
`launcher/webview-server.py`, `scripts/lib/webview-install.sh`, `install.sh`,
`docs/webview-server-evaluation.md`, `docs/usage/troubleshooting.md`,
`tests/scripts_smoke.sh`, `tests/webview_probe_equivalence.sh`.

**Preservation checks:** Use `docs/webview-server-evaluation.md` before
changing the local server model, port behavior, or warm-start adoption.

### 11. Linux Computer Use Integration Compatibility

**Fork delta:** Upstream's Linux Computer Use backend and bundled plugin remain
part of the packaged app. This fork preserves the
`codex-app` package identity, keeps the plugin manifest pointed at packaged
assets, carries local Linux input/window-targeting hardening where needed,
adapts configurable backend identity under the packaged resource layout, and
documents the local opt-in for Computer Use UI patching without claiming that
local installation changes OpenAI account policy or server-side availability.

**Upstream baseline:** The Rust MCP backend, bundled plugin resources,
accessibility tree capture, screenshot paths, and input automation come from
upstream in the synced baseline.

**Why it matters:** The package can stage local Computer Use support and register
the backend on Linux, but useful operation still depends on host accessibility,
screenshot, and input prerequisites. Local UI opt-in controls fork-side patching
only; it is not a server-side entitlement change.

**Current paths:** `computer-use-linux/src/`,
`plugins/openai-bundled/plugins/computer-use/`,
`scripts/patch-linux-window-ui.js`, `scripts/patch-linux-window-ui.test.js`,
`scripts/lib/package-common.sh`, `launcher/start.sh.template`, `README.md`,
`docs/usage/build-and-run.md`, `CHANGELOG.md`.

**Preservation checks:** Keep package staging and README wording scoped to the
local compatibility delta, preserve the `codex-app/settings.json` setting path,
and clear that local installation does not bypass OpenAI feature flags.

### 12. Release, Security, And Supply-Chain Verification

**Fork delta:** The fork adds and wires release/security workflow around the
mutable official OpenAI Codex DMG: trusted DMG hash input, packaged trusted DMG
metadata for unattended updater rebuilds, generated app and ASAR inspection,
package metadata checks, checksums, optional detached checksum signing, public
key export, macOS Apple DMG verification, reviewed hash-refresh PRs, safer DMG
URL validation, download limits, partial-file downloads, and sanitized URL
logging.

**Upstream baseline:** Upstream already downloads and converts the official
OpenAI Codex DMG. This fork adds extra verification and review gates around
that inherited supply chain.

**Why it matters:** This fork rebuilds a package from a mutable official OpenAI
Codex DMG URL. Release and updater work must leave reviewable evidence and
avoid presenting unverified artifacts as trusted.

**Current paths:** `.github/workflows/update-codex-hash.yml`,
`.github/workflows/verify-apple-dmg.yml`, `.github/workflows/ci.yml`,
`.github/workflows/updater.yml`, `Makefile`, `flake.nix`,
`scripts/release-gate.sh`, `scripts/verify-apple-dmg.sh`,
`scripts/inspect-electron-security.js`, `scripts/lib/dmg.sh`,
`updater/trusted-dmg-manifest.json`, `updater/src/trust.rs`,
`updater/src/dmg_source.rs`, `updater/src/app.rs`,
`docs/maintainers/security-backlog.md`, `docs/maintainers/threat-model.md`.

**Preservation checks:** `make help` must show `apple-dmg-verify` and
`release-gate`. Security backlog items that change trust boundaries should use
the `@codex-security` workflow before review-ready handoff.

### 13. User-Local Install Experiment Identity And Layout

**Fork delta:** The experimental unprivileged install path uses `codex-app`
commands, service/timer names, desktop entry, and XDG user data paths. It stays
aligned with fork path triage while remaining separate from native package
layout.

**Upstream baseline:** Upstream already has the user-local install experiment.
This fork renames it and adjusts path choices so it does not reintroduce
upstream names or non-XDG roots.

**Why it matters:** The rootless experiment should not reintroduce upstream
names or non-XDG paths while testing a different install model.

**Current paths:** `contrib/user-local-install/README.md`,
`contrib/user-local-install/install-user-local.sh`,
`contrib/user-local-install/files/.config/systemd/user/`,
`contrib/user-local-install/files/.local/bin/`,
`contrib/user-local-install/files/.local/share/applications/`,
`contrib/user-local-install/files/share/common.sh`.

**Preservation checks:** Keep the payload under
`${XDG_DATA_HOME:-~/.local/share}/codex-app`; do not use `~/.local/opt`.

### 14. Maintainer Policy, Docs, And Agent Workflow

**Fork delta:** The fork adds and maintains policy/docs surfaces that are not
part of upstream: always-loaded agent rules, a repo-local maintenance skill,
maintainer references, security backlog, threat model, usage docs, README
feature status, and the divergence inventory itself.

**Upstream baseline:** These docs should preserve clear credit for upstream's
primary Linux work while describing the local policy and documentation layer as
fork finishing work.

**Why it matters:** This fork is intentionally divergent from its upstream.
Future maintainers and agents need durable, discoverable policy without turning
the README or `AGENTS.md` into large maintenance manuals.

**Current paths:** `AGENTS.md`,
`.agents/skills/maintaining-codex-app-package/SKILL.md`, `docs/README.md`,
`docs/backlog.md`, `docs/maintainers/package-runtime-maintenance.md`,
`docs/maintainers/security-backlog.md`, `docs/maintainers/threat-model.md`,
`docs/policies/agentic-maintenance.md`, `docs/usage/`, `README.md`,
`CHANGELOG.md`.

**Preservation checks:** Keep `AGENTS.md` short and route details to maintainer
docs or repo-local skills. Check README audience, clone URLs, maintainer-only
material, upstream credit, and divergence accuracy before merging
sync PRs.

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
