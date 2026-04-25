# Security Posture Review

Date: 2026-04-25

## Executive Summary

This repository's highest-risk exposure is supply-chain and local privilege flow, not a classic internet-facing web-app surface. The app downloads mutable upstream artifacts, converts and patches an Electron bundle, rebuilds native packages locally, and can install them through `pkexec` after the app exits. The strongest controls today are local-only operation, Rust/Cargo and Nix lockfiles, argument-based subprocess calls, numeric package-version validation, an unprivileged updater daemon boundary, PR-gated Nix hash refreshes, and loopback-only webview serving. The largest remaining gaps are missing authenticated artifact verification in the non-Nix/update path, globally disabled Electron sandboxing, fixed-port local webview spoofing, privileged install subcommands that are not bound tightly enough to updater-generated package artifacts, and public artifact signing/provenance.

Current Electron documentation was fetched through `ctx7` after resolving `/electron/electron`. The fetched guidance reinforces the Electron-related findings: enable renderer sandboxing, keep `webSecurity` enabled, avoid webview Node.js integration, validate `will-attach-webview` options, and rely on isolated preload patterns.

## Critical Findings

None identified in the tracked source review. The review did not include generated `codex-app/` or `Codex.dmg`; both were absent from this checkout during verification, so upstream Electron `webPreferences`, IPC handlers, CSP, and code-signing state remain open items.

## High Findings

### H-1: Electron launches with Chromium sandbox disabled

- Location: [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:685)
- Evidence: the generated launcher execs Electron with `--no-sandbox` and `--disable-gpu-sandbox`.
- Impact: a renderer, webview, or malicious local-origin compromise has a much weaker containment boundary and runs with the user's account privileges.
- Current controls: none visible in the launcher. Generated upstream `BrowserWindow` settings were not inspectable because `codex-app/` is absent.
- Recommendation: make sandboxed launch the default; configure Electron's Linux sandbox support in packaged installs. If sandbox disablement is unavoidable for a subset of systems, gate it behind an explicit opt-out and document the reduced security model.

### H-2: Mutable upstream/update payloads are not authenticated before rebuild and install

- Location: [updater/src/config.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/config.rs:85), [updater/src/upstream.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/upstream.rs:70), [updater/src/app.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/app.rs:264), [updater/src/builder.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/builder.rs:73)
- Evidence: the updater downloads `https://persistent.oaistatic.com/codex-app-prod/Codex.dmg`, computes SHA-256 after receipt, then uses that hash for change detection/workspace naming rather than verifying it against signed or maintainer-approved metadata.
- Impact: compromise of the upstream distribution path, CDN, local trust store, or configured URL can feed arbitrary app content into a local rebuild and eventual privileged native-package install.
- Current controls: HTTPS transport, post-download SHA-256 recording, Nix `fetchurl` fixed hash for the Nix path only.
- Recommendation: add an authenticated update manifest containing version, hash, and signature, with a pinned verification key or equivalent trusted metadata. For public distribution, require maintainer review before accepting a new upstream hash. If the upstream DMG is Apple-signed/notarized, explicitly verify that signature before extraction and record the result; do not treat signing as present unless this repo enforces it.

### H-3: Privileged install subcommands accept caller-supplied package paths

- Location: [updater/src/cli.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/cli.rs:31), [updater/src/app.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/app.rs:49), [updater/src/install.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/install.rs:227)
- Evidence: `install-deb`, `install-rpm`, and `install-pacman` still accept caller-supplied `--path` values, but now reject symlink/non-file inputs, require expected `codex-app` package filename shapes, copy the candidate into a private temp directory, and install that staged copy. Debian and pacman paths get version checks; RPM still lacks package-metadata parity.
- Impact: a user who can satisfy `pkexec` for `codex-app-updater` can install an arbitrary package path through the updater binary rather than only the updater-generated artifact. If a future polkit policy narrows authorization by command instead of artifact, this becomes a stronger local privilege escalation primitive.
- Current controls: `pkexec` prompts for privileged install, package manager arguments are passed without shell interpolation, staged-copy install reduces source replacement races, and Debian/pacman reject non-newer candidates.
- Recommendation: bind privileged install to a verified updater artifact. Validate package identity, architecture, version, canonical path, and expected digest against root-trusted state; add RPM metadata parity.

### H-4: CI hash refresh still needs stronger verification evidence

- Location: [.github/workflows/update-codex-hash.yml](/home/nisavid/src/nisavid/codex-app-linux/.github/workflows/update-codex-hash.yml:8)
- Evidence: the workflow downloads the mutable DMG, computes a new SRI hash, edits `flake.nix`, commits on a bot branch, and opens or updates a PR. Workflow actions are pinned to full commit SHAs.
- Impact: maintainer review now gates Nix trust-root changes, but reviewers still lack automated upstream version/build metadata and code-signing/notarization verification output.
- Current controls: computed SRI syntax validation, Nix fixed-output hash after merge, PR review gate, and commit-pinned workflow actions.
- Recommendation: include upstream version/build metadata and code-signing/notarization verification output in the PR body before accepting public distribution hash changes.

## Medium Findings

### M-1: Fixed local webview server can still be spoofed locally

- Location: [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:577), [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:628), [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:633), [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:649)
- Evidence: the launcher now starts `python3 -m http.server --bind 127.0.0.1 5175` from `content/webview` and no longer kills matching processes with `pkill -f`. It still checks TCP readiness and validates two marker strings from `http://127.0.0.1:5175/index.html`.
- Impact: LAN exposure and broad process-kill risk are reduced, but a local process can still race/occupy port `5175` with marker-matching malicious content. Combined with H-1, the impact rises.
- Recommendation: use an ephemeral loopback port plus per-launch nonce if the upstream app can accept it; validate a generated manifest/hash for critical assets.

### M-2: Package install validation has a TOCTOU window

- Location: [updater/src/install.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/install.rs:234), [updater/src/install.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/install.rs:267), [updater/src/app.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/app.rs:329)
- Evidence: package candidates are now copied into a private temp directory before metadata validation and package-manager execution. The original source path still comes from user-writable cache/state, and no root-trusted digest is checked.
- Impact: source replacement between validation and package-manager consumption is reduced, but a caller who can satisfy `pkexec` can still present a different valid-looking `codex-app` package path.
- Recommendation: persist a trusted expected digest/identity for updater-generated artifacts and re-check it against the staged copy immediately before install.

### M-3: Updater rebuild inherits user-controlled build environment

- Location: [packaging/linux/packaged-runtime.sh](/home/nisavid/src/nisavid/codex-app-linux/packaging/linux/packaged-runtime.sh:16), [updater/src/builder.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/builder.rs:73)
- Evidence: the packaged launcher imports `PATH` into the user systemd manager, and the updater passes inherited `PATH` to `install.sh` and package build scripts.
- Impact: user-writable PATH entries can influence `npm`, `npx`, `curl`, `unzip`, package builders, or other tools used to create a package later offered to `pkexec`.
- Recommendation: set a fixed build PATH such as `/usr/local/sbin:/usr/local/bin:/usr/bin:/bin` or distro-appropriate equivalent for updater rebuilds; avoid importing `PATH` into the service environment, or separate GUI/session env from build env.

### M-4: User-controlled runtime config can redirect the update supply chain

- Location: [updater/src/config.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/config.rs:13), [updater/src/builder.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/builder.rs:69)
- Evidence: config supports arbitrary `dmg_url`, `workspace_root`, and `builder_bundle_root`; builder scripts are copied and executed from `builder_bundle_root`.
- Impact: packaged production mode can be redirected to untrusted builders or payloads by user config, blurring the boundary between supported update behavior and developer override behavior.
- Recommendation: lock packaged updater mode to root-owned `/usr/lib/codex-app/update-builder`; allow custom builder roots only under explicit developer mode; validate canonical paths, ownership, and permissions.

### M-5: Package payload preserves generated app modes and symlinks without normalization

- Location: [scripts/lib/package-common.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/lib/package-common.sh:104), [scripts/build-rpm.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/build-rpm.sh:80)
- Evidence: packaging uses `cp -aT` or `cp -a` from generated app trees into package roots.
- Impact: DMG/npm-derived content can carry unexpected symlinks, executable modes, setuid/setgid bits, or writable files into a root-owned package payload.
- Recommendation: reject absolute/upward symlinks, strip setuid/setgid/world-writable bits, normalize file and directory modes, and fail package builds on unexpected metadata.

### M-6: Non-Nix installer and dependency bootstrap fetch executable inputs without pinned integrity

- Location: [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:307), [install.sh](/home/nisavid/src/nisavid/codex-app-linux/install.sh:369), [scripts/install-deps.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/install-deps.sh:128), [scripts/install-deps.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/install-deps.sh:166)
- Evidence: `npm install`, `npx --yes`, Electron zip downloads, 7zz tarball downloads, and `rustup` shell bootstrap rely on live registries/endpoints.
- Impact: local package builds are less reproducible and more exposed to registry, mirror, or CDN compromise.
- Recommendation: pin Electron zip hashes, `asar`, `@electron/rebuild`, and native rebuild tooling through checked-in manifests/lockfiles; pin 7zz checksums by version and architecture; avoid piping remote shell where a distro package or verified installer is viable.

### M-7: User service lacks systemd sandboxing

- Location: [packaging/linux/codex-app-updater.service](/home/nisavid/src/nisavid/codex-app-linux/packaging/linux/codex-app-updater.service:6)
- Evidence: the service only defines `ExecStart`, restart policy, and network ordering.
- Impact: an updater compromise gets the full ambient user-service environment and filesystem access.
- Recommendation: add compatible hardening such as `NoNewPrivileges=yes`, `PrivateTmp=yes`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, a fixed `Environment=PATH=...`, and the narrowest feasible filesystem protections with explicit writable XDG paths.

### M-8: Native packages lack signing, attestations, and provenance for public distribution

- Location: [scripts/build-deb.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/build-deb.sh:77), [scripts/build-rpm.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/build-rpm.sh:140), [scripts/build-pacman.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/build-pacman.sh:86)
- Evidence: builders emit packages, but no signing/checksum/provenance workflow is present. Pacman build uses `--skipinteg` because there are no remote sources in the local PKGBUILD.
- Impact: public users cannot independently verify artifact origin or detect post-build tampering.
- Recommendation: publish checksums, sign RPM/pacman artifacts, consider minisign/cosign or dpkg signing where appropriate, and add GitHub artifact attestations for release builds.

### M-9: Runtime CLI auto-upgrade trusts latest npm state

- Location: [updater/src/codex_cli.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/codex_cli.rs:295)
- Evidence: updater reads latest `@openai/codex` through `npm view`, then runs `npm install -g @openai/codex@<latest>` or falls back to `--prefix ~/.local`.
- Impact: npm account/registry compromise or unexpected upstream release is pulled into user runtime without a repo-reviewed allowlist.
- Recommendation: keep missing-CLI install interactive, require consent for upgrades, or verify npm package provenance/signatures and allowlist approved versions.

### M-10: Updater download DoS controls are partially addressed

- Location: [updater/src/app.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/app.rs:239), [updater/src/upstream.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/upstream.rs:85), [updater/src/upstream.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/upstream.rs:96)
- Evidence: the updater now configures a 10-minute request timeout, rejects oversized `Content-Length`, enforces a maximum streamed byte count, writes to a temp file, and renames only after size and hash completion.
- Impact: a compromised or malfunctioning endpoint has less ability to keep the updater busy or fill user cache storage. This does not authenticate the downloaded artifact.
- Recommendation: keep the timeout and size cap under test, and add signature or trusted-manifest validation before using the downloaded DMG for rebuild/install.

## Low Findings

### L-1: ASAR file-manager patch opens renderer-provided paths without visible policy checks

- Location: [scripts/patch-linux-window-ui.js](/home/nisavid/src/nisavid/codex-app-linux/scripts/patch-linux-window-ui.js:85)
- Evidence: the injected Linux handler calls `shell.openPath` on a path transformed from renderer-provided input.
- Impact: if untrusted webview content can reach this command path, XSS could trigger local file/folder opens.
- Recommendation: inspect generated app IPC and sender-origin validation once `codex-app/` exists; constrain command shape and reject URLs/control characters.

### L-2: Full URLs and subprocess stderr can persist to state/logs

- Location: [updater/src/upstream.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/upstream.rs:32), [updater/src/app.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/app.rs:311), [updater/src/logging.rs](/home/nisavid/src/nisavid/codex-app-linux/updater/src/logging.rs:9)
- Evidence: upstream errors include full configured URLs; failures are stored in state and appended to service logs.
- Impact: if a user configures URLs with credentials or sensitive query strings, those values can be persisted.
- Recommendation: reject URL userinfo and redact query tokens before writing logs/state.

### L-3: Package template identifiers are not validated before template insertion

- Location: [scripts/build-pacman.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/build-pacman.sh:17), [scripts/build-rpm.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/build-rpm.sh:123), [scripts/lib/package-common.sh](/home/nisavid/src/nisavid/codex-app-linux/scripts/lib/package-common.sh:153)
- Evidence: env-controlled package names and launcher names are inserted into paths, sed replacements, and generated launcher stubs.
- Impact: mostly a local maintainer footgun today, but public packaging should reject unsafe names early.
- Recommendation: validate package and launcher identifiers against distro-safe basename regexes and reject `/`, whitespace, shell metacharacters, and newlines.

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

1. Restore Electron sandboxing or make disablement an explicit, documented fallback.
2. Require authenticated upstream artifact verification before rebuild/install.
3. Bind privileged install subcommands to verified updater artifacts and add RPM metadata parity.
4. Add upstream version/build metadata and signature/notarization verification to hash-update PRs.
5. Reduce fixed-port webview spoofing with a per-launch nonce or ephemeral loopback port.
6. Sanitize updater build environment and package payload metadata.
7. Add package signing/provenance for public distribution.
