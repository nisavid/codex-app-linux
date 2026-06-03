# Experimental Remote Mobile Control

This default-on integration patches the official Codex app main bundle so Linux can
try the mobile remote-control host and outbound control flows that the official app
currently limits to macOS.

Disable it by adding the integration id to `port-integrations/integrations.json` before
building:

```json
{
  "disabled": [
    "remote-mobile-control"
  ]
}
```

For the Nix flake build, use the declarative app variant instead because the
git-ignored `integrations.json` file is not part of the flake source:

```bash
nix run .#remote-mobile-control
```

Integration-specific Nix outputs are additive. To combine this integration with the
Computer Use UI opt-in:

```bash
nix run .#computer-use-ui-remote-mobile-control
```

What it changes:

- Replaces the macOS-only `remote-control-device-key.node` requirement with a
  Linux JavaScript ECDSA P-256 key provider.
- Lets the remote-control Connections UI render on Linux when the official app
  marks the feature unavailable or withholds the remote-control visibility
  rollout.
- Keeps the `Control other devices` settings tab reachable on Linux so this
  desktop can authorize outbound control of another enrolled device.
- Refreshes the remote Connections settings state every 5 seconds and
  immediately after focus, visibility, online, or resume signals.
- Keeps Chrome Browser Use available to remote/mobile controlled sessions when
  the local Chrome plugin and native host are healthy, and adds a diagnostic
  when the native browser bridge is not exposed to the session.
- Persists the private key material at
  `~/.config/codex-app/remote-control-device-keys-v1.json` with `0600`
  file permissions.
- Preserves `remote_control = true` / `features.remote_control = true` in the
  local Codex config instead of letting the official app strip it before
  app-server startup.
- Updates remote-control settings and Codex mobile setup copy so the Linux flow
  is not described as Mac-only.
- Stages `.codex-linux/cold-start.d/remote-mobile-control`, an integration-owned
  cold-start hook that provisions the official managed standalone daemon runtime
  when it is missing, then starts the managed app-server daemon with
  `remote-control start`.

Remote mobile daemon requirement:

The interactive Codex CLI and the remote-control daemon are separate concerns.
You can keep using a Homebrew-installed `codex` for normal terminal and Desktop
app-server usage, but Android remote control currently expects the official
managed standalone daemon runtime at:

```bash
~/.codex/packages/standalone/current/codex
```

If that binary is missing, the integration's cold-start hook runs the official
standalone installer with `CODEX_INSTALL_DIR` pointed at a private bin directory
under `~/.codex/packages/standalone/.bin`. That satisfies the managed daemon
layout without changing `CODEX_CLI_PATH`, creating `~/.local/bin/codex`, or
adding PATH blocks to your shell profile.

The hook is launched best-effort in the background by the generic launcher hook
runner. When the system `timeout` command is available, the installer/start path
is capped by
`CODEX_REMOTE_CONTROL_DAEMON_AUTOSTART_TIMEOUT_SECONDS` (default `30`), so
Desktop cold start is not blocked by network, GitHub, or installer stalls.
When `timeout` is unavailable, the hook continues the installer/start path in a
background subprocess. Hook output is written to the launcher log.

On NixOS, prefer the flake's Home Manager module instead of the launcher hook:

```nix
{
  imports = [
    inputs.codex-app-linux.homeManagerModules.default
  ];

  programs.codexAppLinux = {
    enable = true;
    computerUseUi.enable = true;
    remoteMobileControl.enable = true;
    remoteControl.enable = true;
  };
}
```

The module installs the remote-mobile package variant and manages
`codex-remote-control.service` as a user systemd unit running
`codex app-server --remote-control --listen unix://`. It also sets
`CODEX_REMOTE_CONTROL_DAEMON_AUTOSTART_DISABLED=1` so the launcher does not
start a second mutable standalone daemon.

This is compatible with immutable Linux systems such as Bluefin / Universal
Blue because the managed daemon runtime is user-scoped state under
`~/.codex/packages/standalone`. It does not require `dnf`, `rpm-ostree`, host
package layering, or base-OS mutation. The private `.bin` directory is only a
launcher-owned target for the installer symlink; it is not prepended to the
user's persistent shell `PATH`.

Set `CODEX_REMOTE_CONTROL_RUNTIME_AUTO_INSTALL_DISABLED=1` to disable that
runtime provisioning and only use an already-installed standalone runtime.
Set `CODEX_REMOTE_CONTROL_INSTALLER_SHA256` to require the downloaded official
standalone installer script to match a pinned SHA-256 digest before it runs.

To force a specific daemon binary without affecting the interactive CLI, set:

```bash
CODEX_REMOTE_CONTROL_CODEX_PATH=/path/to/standalone/codex
```

To keep Desktop using Homebrew while the daemon uses standalone, set
`CODEX_CLI_PATH` to the Brew binary and leave
`CODEX_REMOTE_CONTROL_CODEX_PATH` unset or pointed at the standalone binary.

KDE Plasma smoke check:

Mobile control depends on the Linux Computer Use backend once the host is
enrolled. On Plasma/Wayland, verify that the KWin backend is ready after
building or installing the package:

```bash
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux doctor
./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux windows
```

The doctor report should show the KWin window backend, XDG Desktop Portal, and
input checks as ready. The windows report should return `"backend": "kwin"` with
a non-empty `windows` list.

Known risks:

- This is not equivalent to macOS Secure Enclave-backed storage. Private key
  material is file-backed and protected by ordinary user file permissions.
- OpenAI may still reject Linux host enrollment server-side. This integration only
  removes local macOS-only blockers in the repackaged app.
- OpenAI may still reject Linux host enrollment or outbound authorization
  server-side. This integration only removes local macOS-only blockers in the
  repackaged app.
- Treat this as experimental account-level remote-control plumbing.

Run the integration tests with:

```bash
node --test port-integrations/remote-mobile-control/test.js
```
