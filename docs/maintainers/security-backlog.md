# Security Backlog

This backlog tracks security work that is not yet fully addressed by the
current package, launcher, updater, and release pipeline. It is the canonical
handoff surface for security follow-up. Keep point-in-time review reports out
of the repo root once their actionable findings are represented here.

For the broader trust-boundary model, see [Threat Model](threat-model.md).

## Security Review Workflow

Use the `@codex-security` plugin (`plugin://codex-security@openai-curated`) for
security-sensitive backlog work before implementation is treated as
review-ready. This applies especially to updater trust, privileged install
boundaries, release verification, local rebuild inputs, generated-app IPC,
bundled browser or Chrome native-host behavior, Computer Use desktop control,
and secret redaction.

Expected workflow:

1. Run the plugin against the current branch and the relevant backlog item.
2. Record the reviewed trust boundaries, attacker capabilities, and required
   mitigations in the PR body or a maintainer note.
3. Implement the change in source scripts, package templates, updater code, or
   verification workflows rather than generated artifacts.
4. Run the local validation gate for the touched surface, including local app
   generation and package build checks when package or rebuild behavior changes.
5. Re-run `@codex-security` or document why the previous result still applies
   before merging.

`@codex-security` is an additional security review gate. It does not replace
the local build gate, CodeQL, package metadata inspection, threat-model updates,
or human maintainer approval where those are required.

## Highest Priority

### Authenticate updater DMG inputs before rebuild and install

The updater downloads the mutable upstream `Codex.dmg`, hashes the received
bytes, and uses the hash for change detection and workspace naming. It does not
verify the DMG against a signed manifest, pinned maintainer-approved metadata,
or an equivalent trusted update channel before rebuilding a native package.

Before implementing this item, run the `@codex-security` workflow above against
the proposed trust metadata design and updater state transitions.

Desired state:

- the updater accepts only DMGs whose version and digest are authenticated by a
  repo-trusted signing key or equivalent trusted metadata;
- failed or unavailable verification prevents rebuild and install;
- the updater records the version, digest, and verification result in state and
  logs without storing secrets.

### Bind privileged installs to verified updater artifacts

Privileged install subcommands accept caller-supplied package paths. They reject
symlinks and non-files, require expected `codex-app` package names, stage a
private copy, and validate package metadata, but they are not yet bound to a
root-trusted digest for the updater-generated package.

Before implementing this item, run the `@codex-security` workflow above against
the package binding design and the privileged install command surface.

Desired state:

- the unprivileged updater records the expected package digest and identity for
  the generated artifact;
- the privileged install path verifies the staged package against that trusted
  binding immediately before invoking the system package manager;
- package paths are canonicalized under the expected updater workspace where
  that does not break supported manual install flows.

### Add verification evidence to hash-refresh PRs

The scheduled hash-refresh workflow opens a PR instead of writing directly to
`main`. The PR includes the refreshed Nix SRI hashes. The repository also has a
separate Apple DMG verification workflow. The hash-refresh PR body still needs
machine-produced upstream version/build and Apple signature/notarization
evidence before maintainers accept a changed Nix trust root.

Desired state:

- hash-refresh PRs include upstream version/build metadata;
- hash-refresh PRs include the Apple DMG verification result or a link to the
  matching workflow run;
- the workflow does not present a new fixed-output hash as review-ready when
  upstream trust checks are missing or failed.

## Medium Priority

### Reduce local webview spoofing risk

The launcher serves extracted webview assets on fixed loopback port `5175` and
validates marker strings before Electron starts. This avoids LAN exposure and
detects many stale or wrong-port cases, but another same-user process can still
occupy the fixed port with marker-matching content.

Desired state:

- the launcher uses an ephemeral loopback port and a per-launch nonce when the
  upstream app can accept it; or
- critical served assets are validated against generated hashes before launch.

### Review generated-app Electron, IPC, and file-manager handling

The Linux file-manager ASAR patch opens paths through Electron shell APIs when
the upstream bundle shape still matches. The patch is fail-soft, but each
public release candidate still needs generated-app evidence for IPC,
sender-origin validation, navigation, CSP, Electron `webPreferences`, and
file-manager command constraints.

Desired state:

- generated-app review verifies `contextIsolation`, `nodeIntegration`,
  renderer sandboxing, navigation/window handling, CSP, IPC sender validation,
  and `openExternal`/`openPath` policy;
- file-manager inputs reject URLs, control characters, and unexpected command
  shapes before shell APIs are reached;
- the release gate or PR notes identify the generated app bundle and DMG
  evidence used for the review.

### Review Linux Computer Use desktop-control boundary

Linux Computer Use support now has clearer user-facing docs and readiness
checks, but the backend can inspect accessibility trees, capture screenshots,
and synthesize input through host desktop facilities. Local UI opt-in controls
fork-side patching only; account-side policy and host prerequisites remain
separate gates.

Desired state:

- `@codex-security` reviews plugin manifests, MCP command routing, screenshot
  capture, AT-SPI/window selection, and ydotool or portal input paths before
  Computer Use changes are treated as review-ready;
- backend logs and state avoid persisting screenshots, accessibility payloads,
  or credential-looking data beyond the intended local runtime;
- docs and diagnostics keep local opt-in, account availability, host
  accessibility, and desktop input permissions distinct.

### Review bundled browser and Chrome native-host boundary

The generated app stages Browser Use resources and the upstream Chrome plugin
with Linux native-messaging support for Chrome, Brave, and Chromium. These
components run in the user's desktop session and bridge browser state into
Codex through local plugin and native-host paths.

Desired state:

- native-messaging manifests and host paths are restricted to packaged assets
  and expected extension identities;
- browser profile discovery and launch commands use argument vectors and
  sanitized inputs;
- Browser Use and Chrome plugin logs avoid persisting page data, tokens, or
  browser profile paths longer than needed;
- stale browser, CDP, or native-host clients cannot receive unintended future
  commands.

### Require trusted metadata for non-default DMG sources

Runtime config can redirect `dmg_url` for development and testing. URL parsing
rejects userinfo and non-HTTPS non-loopback URLs, but non-default remote
sources are still supply-chain inputs.

Desired state:

- non-default remote `dmg_url` values require the same trusted metadata or
  explicit developer-mode handling as the default update channel;
- logs identify non-default update hosts without persisting secrets.

### Pin executable build inputs outside the Nix path

Non-Nix builds fetch npm packages, Electron archives, 7zz archives, and the
Rust bootstrap through live endpoints. Some helper fallbacks now carry checked
digests, but the broader non-Nix path still relies heavily on TLS, registry
behavior, and operator review.

Desired state:

- Electron archives and helper downloads have checked integrity metadata;
- npm-based build helpers are pinned through checked-in manifests or an
  equivalent reproducible tool path;
- remote shell bootstraps are avoided when a distro package or verified
  installer is viable.

### Harden the updater user service filesystem surface

The packaged user service uses a constrained `PATH`, `PrivateTmp=yes`,
`RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, and `UMask=077`.
`NoNewPrivileges` remains unset because the daemon must invoke `pkexec`.

Desired state:

- filesystem protections are narrowed around explicit XDG config, state, cache,
  and build workspace paths;
- update, rebuild, and install flows are tested under those restrictions before
  enabling them in packages.

### Add public package signing and provenance

The release gate writes `SHA256SUMS` and can require a detached checksum
signature, and signed gates export the public release key. Public release
publishing, format-native signing, and hosted provenance are not yet part of
the package builders or release workflow.

Desired state:

- public artifacts consistently publish `SHA256SUMS`, `SHA256SUMS.asc`, and
  the release signing key;
- public release builds add GitHub artifact attestations or equivalent hosted
  provenance;
- Debian, RPM, and pacman artifacts use format-native signing where practical.

### Review npm CLI auto-upgrade trust

The launcher/updater preflight can query npm for the latest `@openai/codex`
version and install that exact version globally or under `~/.local`. Missing
CLI installation is interactive, but upgrades still trust npm latest-state.

Desired state:

- upgrades require explicit user consent or an approved-version channel;
- npm package provenance or signatures are verified where available;
- the selected CLI version and verification result are logged.

## Lower Priority

### Review Linux open-target discovery and desktop-entry inputs

Linux open-target discovery launches terminals, IDEs, file managers, and
`.desktop` entries as the user. It uses argument-vector launches and sanitizes
app-internal environment variables, but user-local desktop entries and matching
heuristics remain same-user trust inputs by design.

Desired state:

- review allowlists, `.desktop` parsing, command arguments, icon handling, and
  environment sanitization whenever new target families are added;
- reject URLs, control characters, and unexpected command shapes before desktop
  target commands are reached;
- keep the behavior documented as same-user trust, not a privilege boundary.

### Redact credential-looking subprocess output before persistence

Updater URL handling rejects userinfo and redacts query/fragment values in
updater-generated URL context. Build tools, npm, and package managers can still
print arbitrary stderr, and failure messages may persist to state or service
logs.

Desired state:

- credential-looking tokens in subprocess stderr are redacted before state or
  long-lived logs are written;
- tests cover URL redaction and representative subprocess failure messages.
