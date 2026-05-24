# Security Scan: 2026-05-17 Linux-Port Upstream Sync

Scan target: local Linux-port upstream sync merge from
`43c8bd1b5d4ab2eb4be8eb474528d6050c51db9a` to Linux-port upstream commit
`ccaa31fb16217f5706ee1f5113445ca475ea4e34`.

Scope:

- Opt-in `remote-control-ui` and `remote-mobile-control` Linux feature patches.
- AppImage packaging and no-updater package paths.
- Linux launch-action socket, second-instance handoff, and shutdown guard
  patching.
- Updater status/CLI reconciliation changes.
- Computer Use window-targeting changes and package/update-builder staging.

Codex Security phases:

- Threat model: refreshed `docs/maintainers/threat-model.md` for the new
  remote-control/mobile and AppImage surfaces.
- Finding discovery: diff-focused review of the changed security surfaces and
  supporting helpers.
- Validation and attack-path analysis: no reportable candidate survived
  discovery, so no exploit validation or attack-path report was produced.

Result: no reportable security findings in this diff.

Security notes:

- The remote-control/mobile features remain opt-in. The patches expose Linux
  plumbing and Linux-specific copy, but do not fabricate connected clients, MFA
  completion, enrollment, access-required, or remote-environment state.
- Linux remote-control device keys are software keys in
  `${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/remote-control-device-keys-v1.json`
  with `0600` mode. Same-user key theft remains a known limitation and is now
  tracked in the threat model and security backlog.
- The AppImage path is a manual local artifact. It deliberately excludes the
  updater service, polkit policy, privileged install helpers, and update-builder
  bundle.
- The launch-action socket remains same-user local IPC through the generated
  launcher path, parses bounded newline-delimited JSON argv payloads, and is
  gated by the Linux warm-start setting. This sync also restores a `before-quit`
  guard for bootstrap-owned second-instance bundles so launch actions do not
  reopen UI during shutdown.
- The update-builder Linux feature config now stages only the sanitized enabled
  feature list. Local comments and disabled-list metadata are not packaged.

Follow-up:

- Treat remote-control/mobile as experimental until the backlog item
  "Review experimental remote-control and Codex mobile host boundary" is closed.
- No separate `security-best-practices` update is needed for this sync; the
  current durable policy surface is the refreshed threat model plus the
  security backlog item above.
