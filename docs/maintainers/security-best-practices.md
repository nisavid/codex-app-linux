# Security Best Practices

This document projects the repository threat model into secure-by-default review
guidance for maintainer changes. Use it with [Threat Model](threat-model.md)
and [Security Backlog](security-backlog.md).

The current default-enabled port integration set makes generated Electron,
webview, and helper-process boundaries the main day-to-day security surface:
Agent Workspaces, AppShots, wrapper updater UI, Copilot reasoning-effort
settings, open-target discovery, remote-control UI, remote-mobile control,
conversation mode, Read Aloud, and Read Aloud MCP.

## Default Rules

- Treat generated renderer UI, webview settings, global state, local storage,
  profile files, permission files, and bridge params as untrusted inputs. They
  can improve the user flow, but they are not security boundaries unless the
  main process, updater, helper runtime, or OpenAI-hosted service enforces the
  same decision.
- Preserve upstream account, rollout, entitlement, and availability gates when a
  patch adds Linux support. A platform branch should expose Linux plumbing; it
  should not turn an upstream service or account policy check into a local-only
  allow.
- Keep local process launch sinks argument-vector based. Validate the executable
  identity, target path, environment, and option-shaped values before reaching
  `spawn`, `execFile`, Electron `shell.openPath`, or generated open-target
  launch code.
- Avoid adding renderer-side HTML, script, or navigation sinks. Prefer generated
  React/JSX or safe DOM APIs, and do not introduce `innerHTML`,
  `insertAdjacentHTML`, `document.write`, `eval`, `new Function`, string
  timeouts, unvalidated `window.location` assignments, or `postMessage("*")`
  patterns unless the source is constant and the reason is documented.
- Treat values read from local state as attacker-controlled even when this fork
  wrote them. Revalidate setting values at the action sink, especially command
  paths, update flags, model preferences, mount paths, browser-data paths, and
  permission policy files.
- Stage sensitive desktop artifacts in private owner-only paths and remove them
  deterministically. Screenshots, accessibility snapshots, browser-session
  copies, device keys, and captured app data must not live directly in shared
  temporary files or verbose logs.
- Keep update authority in `codex-app-updater`. Generated wrapper-update UI may
  show status and collect user intent, but package eligibility, artifact
  identity, digest binding, and privileged install behavior remain updater
  responsibilities.
- Keep OpenAI-hosted service semantics authoritative. Client-side settings for
  Copilot reasoning effort, remote-control visibility, mobile state, or Computer
  Use availability do not prove hosted entitlement, quota, enrollment, MFA, or
  rollout status.
- Route any newly identified security gap that is outside the current PR's
  implementation scope to GitHub Issues and add it to
  [Security Backlog](security-backlog.md). Keep the threat model current when a
  change creates or removes a trust boundary.

## Default-Enabled Integration Review Points

- **Agent Workspaces:** before launching `agent-workspace-linux`, revalidate the
  selected command, permission file, profile JSON, browser-session copy source,
  mount list, and hidden-workspace acknowledgement state. Main-process hardening
  for command selection and acknowledgement binding is tracked in
  [issue #99](https://github.com/nisavid/codex-app-linux/issues/99).
- **AppShots:** preserve the upstream availability flag, keep global hotkeys
  opt-in, fail closed when focused-window inputs are unavailable, and use private
  per-capture temporary directories for screenshot intermediates.
- **Wrapper updater UI:** keep wrapper update checks off until the user enables
  them, avoid UI states that imply a package is verified before updater state
  says so, and leave failed apply markers in a retryable state.
- **Copilot reasoning effort:** treat generated setting defaults as preference
  hints only. Hosted request handling remains authoritative for entitlement,
  quota, and request normalization; validation is tracked in
  [issue #100](https://github.com/nisavid/codex-app-linux/issues/100).
- **Remote-control and mobile host integrations:** do not fabricate connected
  clients, MFA, enrollment, host identity, app-server reachability, or remote
  environment state. Use
  [Remote Mobile Host Boundary Review](remote-mobile-host-boundary-review.md)
  for host-state evidence.
- **Open target discovery:** keep `.desktop` parsing narrow, reject URL-like or
  option-shaped targets before launch, sanitize app-internal environment
  variables, and treat user-local desktop entries as same-user trust inputs.

## Review Checklist

Before default-enabling or materially changing a port integration, confirm:

- The integration's control surface is documented in its README and linked from
  `port-integrations/README.md` when user-facing.
- Runtime controls are enforced at the trusted sink, not only in generated UI.
- Any official app availability or hosted-service gate is preserved or replaced
  by a documented equivalent.
- Sensitive artifacts use private state or temp paths, owner-only modes where
  applicable, and deterministic cleanup.
- Tests cover the security-relevant branch, including stale already-patched
  bundle shapes when the patcher supports upgrades.
- The threat model, security backlog, and maintainer review paths are updated
  when the change creates, removes, or materially shifts a trust boundary.
