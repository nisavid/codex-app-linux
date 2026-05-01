# Security Backlog

This backlog tracks security work that is not yet fully addressed by the
current package, launcher, updater, and release pipeline. It is the canonical
handoff surface for security follow-up. Keep point-in-time review reports out
of the repo root once their actionable findings are represented here.

For the broader trust-boundary model, see [Threat Model](threat-model.md).

## Security Review Workflow

Use the `codex-security` plugin (`plugin://codex-security@openai-curated`) for
security-sensitive backlog work before implementation is treated as review-ready.
This applies especially to updater trust, privileged install boundaries, release
verification, local rebuild inputs, generated-app IPC, and secret redaction.

Expected workflow:

1. Run the plugin against the current branch and the relevant backlog item.
2. Record the reviewed trust boundaries, attacker capabilities, and required
   mitigations in the PR body or a maintainer note.
3. Implement the change in source scripts, package templates, updater code, or
   verification workflows rather than generated artifacts.
4. Run the local validation gate for the touched surface, including local app
   generation and package build checks when package or rebuild behavior changes.
5. Re-run `codex-security` or document why the previous result still applies
   before merging.

`codex-security` is an additional security review gate. It does not replace the
local build gate, CodeQL, package metadata inspection, threat-model updates, or
human maintainer approval where those are required.

## Highest Priority

### Authenticate updater DMG inputs before rebuild and install

The updater downloads the mutable upstream `Codex.dmg`, hashes the received
bytes, and uses the hash for change detection and workspace naming. It does not
verify the DMG against a signed manifest, pinned maintainer-approved metadata,
or an equivalent trusted update channel before rebuilding a native package.

Before implementing this item, run the `codex-security` workflow above against
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

Before implementing this item, run the `codex-security` workflow above against
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
`main`, and the repository has a separate Apple DMG verification workflow. The
hash-refresh PR body still needs machine-produced upstream version/build and
Apple signature/notarization evidence before maintainers accept a changed Nix
trust root.

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
Rust bootstrap through live endpoints. Current controls rely mostly on TLS,
registry behavior, and operator review.

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
signature. Format-native signing and hosted provenance are not yet part of the
package builders or release workflow.

Desired state:

- public artifacts publish `SHA256SUMS`, `SHA256SUMS.asc`, and the release
  signing key;
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

### Review generated-app IPC and file-manager path handling

The Linux file-manager ASAR patch opens paths through Electron shell APIs when
the upstream bundle shape still matches. The patch is fail-soft, but the
generated app's IPC, sender-origin validation, and command-shape constraints
need manual review whenever the patch applies to a new upstream bundle.

Desired state:

- generated-app review verifies `contextIsolation`, `nodeIntegration`,
  renderer sandboxing, navigation/window handling, CSP, IPC sender validation,
  and `openExternal`/`openPath` policy;
- file-manager inputs reject URLs, control characters, and unexpected command
  shapes before shell APIs are reached.

### Redact credential-looking subprocess output before persistence

Updater URL handling rejects userinfo and redacts query/fragment values in
updater-generated URL context. Build tools, npm, and package managers can still
print arbitrary stderr, and failure messages may persist to state or service
logs.

Desired state:

- credential-looking tokens in subprocess stderr are redacted before state or
  long-lived logs are written;
- tests cover URL redaction and representative subprocess failure messages.
