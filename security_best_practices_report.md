# Security Posture Review

Date: 2026-04-25

## Executive Summary

This repository's highest-risk exposure is supply-chain and local privilege flow, not a classic internet-facing web-app surface. The app downloads mutable upstream artifacts, converts and patches an Electron bundle, rebuilds native packages locally, and can install them through `pkexec` after the app exits. The strongest controls today are local-only operation, Rust/Cargo and Nix lockfiles, argument-based subprocess calls, numeric package-version validation, an unprivileged updater daemon boundary, PR-gated Nix hash refreshes, loopback-only webview serving, and sandboxed Electron launch by default. The largest remaining gaps are missing authenticated artifact verification in the non-Nix/update path, fixed-port local webview spoofing, privileged install subcommands that are not bound tightly enough to updater-generated package artifacts, and public artifact signing/provenance.

Current Electron documentation was fetched through `ctx7` after resolving `/electron/electron`. The fetched guidance reinforces the Electron-related findings: enable renderer sandboxing, keep `webSecurity` enabled, avoid webview Node.js integration, validate `will-attach-webview` options, and rely on isolated preload patterns.

## Critical Findings

None identified in the tracked source review. The review did not include generated `codex-app/` or `Codex.dmg`; both were absent from this checkout during verification, so upstream Electron `webPreferences`, IPC handlers, CSP, and code-signing state remain open items. The branch now includes `scripts/inspect-electron-security.js` and `scripts/release-gate.sh` as repeatable gates once generated app and release artifacts exist.

## High Findings

### H-1: Generated Electron app security settings remain unverified

- Location: [install.sh](install.sh:685)
- Evidence: the generated launcher now omits `--no-sandbox` and `--disable-gpu-sandbox` by default. It retains an explicit `CODEX_APP_DISABLE_ELECTRON_SANDBOX=1` compatibility fallback. Generated upstream `BrowserWindow` settings were not inspectable because `codex-app/` is absent. `scripts/inspect-electron-security.js` flags high-confidence static anti-patterns such as `nodeIntegration: true`, `contextIsolation: false`, `sandbox: false`, insecure `<webview>` attributes, and `shell.openExternal` review points.
- Impact: renderer, webview, or malicious local-origin compromise now has a stronger Chromium process boundary by default, but generated app settings and the explicit opt-out still need release-gate review.
- Current controls: sandboxed launch by default, an explicit documented lower-security fallback, and `make release-gate` extracts `resources/app.asar` and runs the generated-app inspector before public artifact publication.
- Recommendation: after building the current generated app, run `make release-gate` plus manual inspection of generated app `webPreferences`, navigation, webview, IPC, CSP, and `openExternal` handling before public release; keep sandbox disablement opt-in only.

### H-2: Mutable upstream/update payloads are not authenticated before rebuild and install

- Location: [updater/src/config.rs](updater/src/config.rs:85), [updater/src/upstream.rs](updater/src/upstream.rs:70), [updater/src/app.rs](updater/src/app.rs:264), [updater/src/builder.rs](updater/src/builder.rs:73)
- Evidence: the updater downloads `https://persistent.oaistatic.com/codex-app-prod/Codex.dmg`, computes SHA-256 after receipt, then uses that hash for change detection/workspace naming rather than verifying it against signed or maintainer-approved metadata.
- Impact: compromise of the upstream distribution path, CDN, local trust store, or configured URL can feed arbitrary app content into a local rebuild and eventual privileged native-package install.
- Current controls: HTTPS transport, post-download SHA-256 recording, Nix `fetchurl` fixed hash for the Nix path, and a public release gate that requires the release DMG to match a reviewed SHA-256/SRI hash before checksums/signatures are produced.
- Recommendation: add an authenticated updater manifest containing version, hash, and signature, with a pinned verification key or equivalent trusted metadata. If the upstream DMG is Apple-signed/notarized, explicitly verify that signature before extraction and record the result; do not treat signing as present unless this repo enforces it.

### H-3: Privileged install subcommands accept caller-supplied package paths

- Location: [updater/src/cli.rs](updater/src/cli.rs:31), [updater/src/app.rs](updater/src/app.rs:49), [updater/src/install.rs](updater/src/install.rs:227)
- Evidence: `install-deb`, `install-rpm`, and `install-pacman` still accept caller-supplied `--path` values, but now reject symlink/non-file inputs, require expected `codex-app` package filename shapes, copy the candidate into a private temp directory, and install that staged copy. RPM metadata is queried with `rpm -qp`, Debian metadata with `dpkg-deb -f`, and pacman metadata with `pacman -Qp`; all three package names must be `codex-app`.
- Impact: a user who can satisfy `pkexec` for `codex-app-updater` can install an arbitrary package path through the updater binary rather than only the updater-generated artifact. If a future polkit policy narrows authorization by command instead of artifact, this becomes a stronger local privilege escalation primitive.
- Current controls: `pkexec` prompts for privileged install, package manager arguments are passed without shell interpolation, staged-copy install reduces source replacement races, package identity is checked from metadata, and Debian/pacman reject non-newer candidates.
- Recommendation: bind privileged install to a verified updater artifact. Validate canonical path and expected digest against root-trusted state.

### H-4: CI hash refresh still needs stronger verification evidence

- Location: [.github/workflows/update-codex-hash.yml](.github/workflows/update-codex-hash.yml:8)
- Evidence: the workflow downloads the mutable DMG, computes a new SRI hash, edits `flake.nix`, commits on a bot branch, and opens or updates a PR. Workflow actions are pinned to full commit SHAs.
- Impact: maintainer review now gates Nix trust-root changes, but reviewers still lack automated upstream version/build metadata and code-signing/notarization verification output.
- Current controls: computed SRI syntax validation, Nix fixed-output hash after merge, PR review gate, and commit-pinned workflow actions.
- Recommendation: include upstream version/build metadata and code-signing/notarization verification output in the PR body before accepting public distribution hash changes.

## Medium Findings

### M-1: Fixed local webview server can still be spoofed locally

- Location: [install.sh](install.sh:577), [install.sh](install.sh:628), [install.sh](install.sh:633), [install.sh](install.sh:649)
- Evidence: the launcher now starts `python3 -m http.server --bind 127.0.0.1 5175` from `content/webview` and no longer kills matching processes with `pkill -f`. It still checks TCP readiness and validates two marker strings from `http://127.0.0.1:5175/index.html`.
- Impact: LAN exposure and broad process-kill risk are reduced, but a local process can still race/occupy port `5175` with marker-matching malicious content. Combined with H-1, the impact rises.
- Recommendation: use an ephemeral loopback port plus per-launch nonce if the upstream app can accept it; validate a generated manifest/hash for critical assets.

### M-2: Package install validation has a TOCTOU window

- Location: [updater/src/install.rs](updater/src/install.rs:234), [updater/src/install.rs](updater/src/install.rs:267), [updater/src/app.rs](updater/src/app.rs:329)
- Evidence: package candidates are now copied into a private temp directory before metadata validation and package-manager execution. The original source path still comes from user-writable cache/state, and no root-trusted digest is checked.
- Impact: source replacement between validation and package-manager consumption is reduced, but a caller who can satisfy `pkexec` can still present a different valid-looking `codex-app` package path.
- Recommendation: persist a trusted expected digest/identity for updater-generated artifacts and re-check it against the staged copy immediately before install.

### M-3: Updater rebuild environment is narrowed, with explicit developer override

- Location: [packaging/linux/packaged-runtime.sh](packaging/linux/packaged-runtime.sh:16), [updater/src/builder.rs](updater/src/builder.rs:73)
- Evidence: the packaged launcher no longer imports the full user session `PATH` into the user systemd manager, updater rebuilds run `install.sh` plus package build scripts with system directories plus `~/.local/bin`, and packaged installs force `builder_bundle_root` to `/usr/lib/codex-app/update-builder` unless `developer_mode = true`. Builder bundle copying now rejects symlinked entries, rejects group/world-writable production roots, and requires the packaged root path to be owned by root.
- Impact: broad user-writable `PATH` entries no longer influence updater rebuild commands, while the documented `~/.local/bin` tool fallback remains available. Builder-root redirects are explicit developer-mode behavior.
- Recommendation: keep builder-root validation covered by updater tests; if developer mode is used, treat the configured builder root as a trusted local development override.

### M-4: User-controlled runtime config can redirect the update supply chain

- Location: [updater/src/config.rs](updater/src/config.rs:13), [updater/src/builder.rs](updater/src/builder.rs:69)
- Evidence: config still supports arbitrary `dmg_url` and `workspace_root`; `builder_bundle_root` requires `developer_mode = true` when the packaged builder root exists. Updater DMG URLs now reject userinfo before requests are made.
- Impact: packaged production mode is less exposed to untrusted builders, but payload source and workspace redirects remain developer-visible supply-chain controls.
- Recommendation: gate non-default `dmg_url` behind equivalent trusted-metadata verification.

### M-5: Package payload normalization does not authenticate generated contents

- Location: [scripts/lib/package-common.sh](scripts/lib/package-common.sh:104), [scripts/build-rpm.sh](scripts/build-rpm.sh:80)
- Evidence: Debian, RPM, and pacman packaging now share app payload staging. Staging rejects absolute or upward symlinks, normalizes generated app directory/file modes, and normalizes package root directory/file modes before package creation, including under the updater service's private `UMask=077`.
- Impact: package metadata risk is reduced, but generated contents still originate from mutable upstream DMG/npm inputs and are not authenticated by this normalization.
- Recommendation: keep symlink/mode checks covered by smoke tests, and pair them with upstream artifact verification before public release.

### M-6: Non-Nix installer and dependency bootstrap fetch executable inputs without pinned integrity

- Location: [install.sh](install.sh:307), [install.sh](install.sh:369), [scripts/install-deps.sh](scripts/install-deps.sh:128), [scripts/install-deps.sh](scripts/install-deps.sh:166)
- Evidence: `npm install`, `npx --yes`, Electron zip downloads, 7zz tarball downloads, and `rustup` shell bootstrap rely on live registries/endpoints.
- Impact: local package builds are less reproducible and more exposed to registry, mirror, or CDN compromise.
- Recommendation: pin Electron zip hashes, `asar`, `@electron/rebuild`, and native rebuild tooling through checked-in manifests/lockfiles; pin 7zz checksums by version and architecture; avoid piping remote shell where a distro package or verified installer is viable.

### M-7: User service hardening is partially applied

- Location: [packaging/linux/codex-app-updater.service](packaging/linux/codex-app-updater.service:6)
- Evidence: the service now sets a constrained `PATH` with system directories plus `%h/.local/bin`, `PrivateTmp=yes`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, and `UMask=077`. `NoNewPrivileges` is intentionally not set because the daemon must invoke `pkexec` for the final install step.
- Impact: an updater compromise gets a narrower user-service environment without blocking privileged install. Broad home-directory access remains because the updater needs XDG config/state/cache and package workspaces.
- Recommendation: evaluate narrower filesystem protections with explicit writable XDG paths once update/install behavior is tested under those restrictions.

### M-8: Native packages lack signing, attestations, and provenance for public distribution

- Location: [scripts/build-deb.sh](scripts/build-deb.sh:77), [scripts/build-rpm.sh](scripts/build-rpm.sh:140), [scripts/build-pacman.sh](scripts/build-pacman.sh:86)
- Evidence: builders emit packages, and `make release-gate` now writes `dist/SHA256SUMS`, can require a detached GPG signature at `dist/SHA256SUMS.asc`, publishes `dist/release-signing-key.asc`, and verifies the signature against that public key. Pacman build uses `--skipinteg` because there are no remote sources in the local PKGBUILD.
- Impact: public users can verify checksums and checksum signature when release maintainers run the gate with `REQUIRE_RELEASE_SIGNATURE=1`. Format-native package signing and hosted provenance remain open.
- Recommendation: require the release gate for public artifacts, publish `SHA256SUMS`, `SHA256SUMS.asc`, and `release-signing-key.asc`, then add format-native signing and GitHub artifact attestations for hosted release builds.

### M-9: Runtime CLI auto-upgrade trusts latest npm state

- Location: [updater/src/codex_cli.rs](updater/src/codex_cli.rs:295)
- Evidence: updater reads latest `@openai/codex` through `npm view`, then runs `npm install -g @openai/codex@<latest>` or falls back to `--prefix ~/.local`.
- Impact: npm account/registry compromise or unexpected upstream release is pulled into user runtime without a repo-reviewed allowlist.
- Recommendation: keep missing-CLI install interactive, require consent for upgrades, or verify npm package provenance/signatures and allowlist approved versions.

### M-10: Updater download DoS controls are partially addressed

- Location: [updater/src/app.rs](updater/src/app.rs:239), [updater/src/upstream.rs](updater/src/upstream.rs:85), [updater/src/upstream.rs](updater/src/upstream.rs:96)
- Evidence: the updater now configures a 10-minute request timeout, rejects oversized `Content-Length`, enforces a maximum streamed byte count, writes to a temp file, and renames only after size and hash completion.
- Impact: a compromised or malfunctioning endpoint has less ability to keep the updater busy or fill user cache storage. This does not authenticate the downloaded artifact.
- Recommendation: keep the timeout and size cap under test, and add signature or trusted-manifest validation before using the downloaded DMG for rebuild/install.

## Low Findings

### L-1: ASAR file-manager patch opens renderer-provided paths without visible policy checks

- Location: [scripts/patch-linux-window-ui.js](scripts/patch-linux-window-ui.js:85)
- Evidence: the injected Linux handler calls `shell.openPath` on a path transformed from renderer-provided input.
- Impact: if untrusted webview content can reach this command path, XSS could trigger local file/folder opens.
- Recommendation: inspect generated app IPC and sender-origin validation once `codex-app/` exists; constrain command shape and reject URLs/control characters.

### L-2: Full URLs and subprocess stderr can persist to state/logs

- Location: [updater/src/upstream.rs](updater/src/upstream.rs:32), [updater/src/app.rs](updater/src/app.rs:311), [updater/src/logging.rs](updater/src/logging.rs:9)
- Evidence: updater DMG URLs now reject userinfo, require HTTPS except for loopback test/dev URLs, and redact query and fragment values in updater-generated request context. Failures are still stored in state and appended to service logs, and subprocess stderr can include arbitrary tool output.
- Impact: credential-bearing DMG URLs are blocked, cleartext remote DMG URLs are rejected, and query/fragment values are less likely to persist through updater request errors. Sensitive subprocess output can still persist if external tools print it.
- Recommendation: keep URL userinfo rejection and query-redaction tests; redact credential-looking subprocess stderr before persisting build/package-manager failures.

### L-3: Package template identifiers now fail closed before template insertion

- Location: [scripts/lib/package-common.sh](scripts/lib/package-common.sh:69), [scripts/build-pacman.sh](scripts/build-pacman.sh:53), [scripts/build-rpm.sh](scripts/build-rpm.sh:52), [scripts/build-deb.sh](scripts/build-deb.sh:50)
- Evidence: env-controlled package names, provides/conflicts values, app install names, and launcher names are validated against distro-safe or basename-safe regexes before staging, template rendering, and launcher stub creation.
- Impact: unsafe `/`, whitespace, shell metacharacters, and newline-bearing identifiers now fail before they can influence paths or generated package templates.
- Recommendation: keep identifier validation in the script smoke suite when adding new package metadata fields.

## Positive Controls

- The updater daemon is unprivileged until final install, and escalation is limited to explicit install subcommands.
- Most subprocess use in Rust passes arguments directly rather than through a shell.
- Generated package versions are constrained to numeric dot-separated segments.
- Rust dependencies are locked in `Cargo.lock`, and Nix inputs are locked in `flake.lock`.
- The Nix path pins the upstream DMG with a fixed-output hash.
- No hardcoded API keys or obvious secret material were found in tracked updater, packaging, workflow, or docs paths.

## External References Consulted

- Context7 `/electron/electron` docs for Electron security and sandbox guidance.
- Electron security checklist: https://www.electronjs.org/docs/latest/tutorial/security
- Electron sandboxing: https://www.electronjs.org/docs/latest/tutorial/sandbox
- OWASP Desktop App Security Top 10: https://owasp.org/www-project-desktop-app-security-top-10/

## Follow-Up Priority

1. Run `make release-gate` and manually inspect generated Electron app security settings before public release.
2. Add authenticated updater artifact verification before rebuild/install.
3. Bind privileged install subcommands to verified updater artifacts with trusted digest and canonical workspace checks.
4. Add upstream version/build metadata and signature/notarization verification to hash-update PRs.
5. Reduce fixed-port webview spoofing with a per-launch nonce or ephemeral loopback port.
6. Sanitize updater build environment and keep package payload metadata checks covered by smoke tests.
7. Add package signing/provenance for public distribution.
