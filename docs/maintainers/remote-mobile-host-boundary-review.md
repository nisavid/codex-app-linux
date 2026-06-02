# Remote Mobile Host Boundary Review

This review note records the current review gate for the `remote-control-ui`
and `remote-mobile-control` port integrations. It is issue #59's durable
evidence surface for the local host boundary; GitHub comments should point back
here instead of carrying the only copy of the matrix.

## Scope

The review covers fork-owned Linux behavior around:

- generated bundle patch descriptors for remote-control and Codex mobile
  surfaces;
- app-server or managed remote-control daemon startup and config preservation;
- the Linux software device-key provider under XDG config;
- host identity selection for remote-control auto-connect;
- docs and review evidence that distinguish local fork behavior from
  OpenAI-hosted account, enrollment, MFA, mobile-client authorization, and
  remote-access policy.

Generated app output, local state, and mobile-client behavior are inspection
evidence. Durable changes belong in source patchers, port integration tests,
maintainer docs, or the issue/PR evidence trail.

## Host-State Matrix

| Review row | Repo-local evidence | Live evidence required before issue closure |
| --- | --- | --- |
| Enrollment state shown in UI | `PRODUCT.md` and `DESIGN.md` require that connected-looking UI not imply unverified enrollment, host liveness, thread visibility, authorization, remote environment state, or service availability. | A live account/mobile check records the enrollment state shown in the generated app and confirms it matches current account/mobile enrollment state. |
| App-server or daemon liveness | `port-integrations/remote-mobile-control/cold-start-hook.sh` starts the managed daemon only when the Desktop app-server does not own remote-control, and tests cover the ownership marker and stale daemon PID cleanup. | A live run records the intended Desktop app-server or managed daemon process as alive and reachable. |
| Linux device-key store | `port-integrations/remote-mobile-control/test.js` covers key creation, signing, deletion, and `0600` mode for `${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/remote-control-device-keys-v1.json`. | A live run records the configured key path and owner-only mode without exposing key material. |
| Mobile side sees intended thread/session | Fork-side tests cannot prove OpenAI-hosted account/mobile discovery semantics. | The mobile side records the intended host thread/session as visible. |
| First mobile action reaches intended host thread/session | Fork-side tests cannot prove mobile action routing through OpenAI-hosted services. | A first mobile action or message is applied to the intended live host thread/session. |
| Stale, revoked, unauthorized, or mismatched hosts are rejected | `port-integrations/remote-mobile-control/test.js` covers auto-connecting only the local installation, leaving hosts disconnected when no local identity is available, and refreshing empty connection snapshots before selecting the intended host. | A live run records stale, revoked, unauthorized, or mismatched hosts rejected instead of displayed as connected. |

## Scoped Security Review Evidence

- `remote-control-ui` patches expose Linux remote-control UI surfaces and Linux
  copy. They do not authorize a host, mint device keys, or prove account/mobile
  enrollment.
- `remote-mobile-control` keeps OpenAI-hosted account, enrollment, step-up, MFA,
  mobile-client authorization, and remote-access decisions outside the local
  fork. Local patches preserve those checks and only adapt Linux host plumbing.
- Linux device keys are exportable software keys. The provider stores them under
  per-user XDG config, writes the key store with `0600` mode, uses a `0700` lock
  directory, and fails when no user config root can be resolved.
- The Desktop app-server path owns remote-control when the generated app carries
  the ownership marker. The standalone daemon path is a local fallback and must
  not imply OpenAI-hosted enrollment or mobile reachability.
- Auto-connect is limited to `remote-control:` host records whose installation
  id matches the local `electron-local-remote-control-installation-id`. Empty
  connection snapshots are refreshed before selection, and missing local
  identity leaves every remote host disconnected.

## Review Rules

- Do not treat a connected-looking local UI as proof of account/mobile
  authorization, host liveness, or thread/session reachability.
- Do not claim general-ready status until `@codex-security` review evidence and
  the host-state matrix are both recorded.
- Keep local fork behavior distinct from OpenAI-hosted services in docs, PR
  text, and issue closure comments.
- Do not persist screenshots, key material, private account identifiers,
  private hostnames, private paths, tokens, or mobile-client state that is not
  necessary for review.

## Local Validation

Run these local checks after source or review-doc changes in this area:

```bash
node --test port-integrations/remote-control-ui/test.js
node --test port-integrations/remote-mobile-control/test.js
```

If shared patching behavior changes, also run:

```bash
node --test scripts/patch-linux-window-ui.test.js
```

If shell hooks change, run `bash -n` on the touched shell files.
