# Port Integrations

`port-integrations/` is the source path for this fork's configurable
port integration registry. A port integration is a build-time integration module that
adapts official Codex app behavior or local runtime helpers to this Linux port.
Integrations are not official Codex plugins and are not necessarily Linux-only
Codex feature concepts. They can add ASAR patches, webview or extracted-app
patches, staged resources, or build/install hooks.

The Linux-port upstream calls this registry "Linux features" and keeps it under
`linux-features/`. This fork uses "port integrations" and `port-integrations/`
because the modules are port-authored integration points, not features of Linux
itself. When reporting an issue upstream, translate this fork's names back to
upstream's names and reproduce with an upstream build when possible.

The registry is not a complete inventory of port-authored behavior. Browser Use
and Linux Computer Use use separate packaging and patching paths. Keep core
compatibility patches in `scripts/patches/` until they are deliberately
migrated. Use `port-integrations/` for configurable integrations whose default can be
changed without moving code into the core patch registry.

## Defaults And Local Overrides

This fork enables the current supported integration set by default:
`agent-workspace`, `appshots`, `codex-wrapper-updater`, `conversation-mode`,
`copilot-reasoning-effort`, `open-target-discovery`, `read-aloud`,
`read-aloud-mcp`, `remote-control-ui`, and `remote-mobile-control`. Open target
discovery improves the Linux Open menus with terminal, editor, and file-manager
targets. It reads the current user's desktop app entries and launches selected
targets as that same user; see
[`open-target-discovery/README.md`](open-target-discovery/README.md) for the
scope and trust notes. Agent Workspaces remains controlled from its settings page
for the normal UI flow; main-process hardening for direct bridge calls is tracked
in [#99](https://github.com/nisavid/codex-app-linux/issues/99). AppShots uses
best-effort focused-window capture, preserves the upstream availability flag, and
keeps global hotkeys inactive until the user selects one. Wrapper update checks
remain off at runtime until the user enables them in Settings. Copilot reasoning
effort defaults only affect Copilot auth sessions; backend entitlement semantics
are tracked in [#100](https://github.com/nisavid/codex-app-linux/issues/100).
The remote control and voice integrations still depend on OpenAI account rollout,
local audio, connected-client state, and host network availability.

To disable a default integration for a checkout build, copy `integrations.example.json`
to the git-ignored `integrations.json`, add the integration id under `disabled`, then
rerun `./install.sh` or the package build:

```json
{
  "enabled": [],
  "disabled": [
    "conversation-mode",
    "agent-workspace",
    "appshots",
    "codex-wrapper-updater",
    "copilot-reasoning-effort",
    "remote-control-ui",
    "remote-mobile-control",
    "read-aloud",
    "read-aloud-mcp"
  ]
}
```

To enable a still-optional integration, list it under `enabled`:

```json
{
  "enabled": [
    "zed-opener"
  ],
  "disabled": []
}
```

You can combine both lists:

```json
{
  "enabled": [
    "copilot-reasoning-effort"
  ],
  "disabled": [
    "open-target-discovery"
  ]
}
```

`disabled` wins if the same integration appears in both lists. `integrations.json` is
ignored by git so local choices do not leak into commits. Integration choices are
read during the install/build pipeline; if you change this file after an app
has already been generated, rerun the install/build step.

Packaged installs and updater rebuilds can use a persistent user override at
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/port-integrations.json` with the same
shape. Checkout builds intentionally ignore that persistent user file when the
repo has a `.git` directory or worktree pointer, so packaged-install preferences
do not silently change local development builds or tests. For one-off builds,
set `CODEX_PORT_INTEGRATIONS_CONFIG=/path/to/file.json` to point at an explicit
config file.
Native packages omit checkout-local integration config from the packaged
update-builder bundle. Updater rebuilds resolve the persistent user override at
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/port-integrations.json`, then fall
back to the default-enabled integration manifests in the bundle.

You can also let the guided native setup helper discover integration manifests and
write `integrations.json`:

```bash
make setup-native

# non-interactive integration edits:
CODEX_BOOTSTRAP_NONINTERACTIVE=1 \
CODEX_PORT_INTEGRATIONS=remote-mobile-control,read-aloud \
CODEX_DISABLE_PORT_INTEGRATIONS=conversation-mode \
make setup-native
```

Disabling an integration in `integrations.json` only affects the next rebuild. The helper
does not delete local device keys, Read Aloud model files, plugin caches, Python
runtimes, or ydotool services. Integration-owned cleanup is a separate interactive
action:

```bash
CODEX_BOOTSTRAP_CLEANUP_INTEGRATIONS=remote-mobile-control,read-aloud make setup-native
```

The helper lists exact paths and deletes only paths confirmed with
`DELETE <exact path>`. Add `CODEX_BOOTSTRAP_DRY_RUN=1` to preview cleanup
targets without deleting them. Legacy `CODEX_LINUX_FEATURES_*` and
`CODEX_BOOTSTRAP_CLEANUP_FEATURES` variables are accepted as compatibility
aliases, but new docs and scripts should use `CODEX_PORT_INTEGRATIONS_*` and
`CODEX_BOOTSTRAP_CLEANUP_INTEGRATIONS`.

Each integration directory should include:

- `integration.json` — metadata, optional `defaultEnabled`, and entrypoints
- `README.md` — what it does, how to test it, and known risks
- optional `patch.js` — exports `applyMainBundlePatch(source, context)`, or
  descriptor patches when `integration.json` uses `entrypoints.patchDescriptors`
- optional `stage.sh` — install/build staging hook
- optional `test.js` — self-contained tests for the integration

`stage.sh` hooks run with `SCRIPT_DIR`, `INSTALL_DIR`, `WORK_DIR`, `ARCH`, and
`CODEX_OFFICIAL_APP_DIR` in the environment. `CODEX_UPSTREAM_APP_DIR` remains a
legacy alias for existing hooks.

Descriptor patches use the same shape as `scripts/patches/core/**/patch.js`.
They can target `main-bundle`, `webview-asset`, or `extracted-app` phases.
Integration descriptor ids are namespaced as `integration:<integration-id>:<descriptor-id>`
in patch reports and are optional by default.

Integration self-tests live inside each integration directory. Run them with:

```bash
node --test port-integrations/*/test.js
```

The manifest file is `integration.json`, and config files use the `enabled` and
`disabled` keys. Treat those names as implementation API, not as the
reader-facing model for what the integration provides.
