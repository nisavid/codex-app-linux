# Linux Features

`linux-features/` contains Linux integration modules for this wrapper. These
are not official Codex plugins; they are Linux-side extensions that can add ASAR
patches, staged resources, or build/install hooks.

## Defaults And Local Overrides

This fork enables the current Linux integration features by default:
`open-target-discovery`, `remote-control-ui`, `remote-mobile-control`,
`read-aloud`, `read-aloud-mcp`, and `conversation-mode`. Open target discovery
improves the Linux Open menus with terminal, editor, and file-manager targets. It
reads the current user's desktop app entries and launches selected targets as
that same user; see
[`open-target-discovery/README.md`](open-target-discovery/README.md) for the
scope and trust notes. The remote control and voice features still depend on
OpenAI account rollout, local audio, connected-client state, and host network
availability.

To disable a default feature for a checkout build, copy `features.example.json`
to the git-ignored `features.json`, add the feature id under `disabled`, then
rerun `./install.sh` or the package build:

```json
{
  "enabled": [],
  "disabled": [
    "conversation-mode",
    "remote-control-ui",
    "remote-mobile-control",
    "read-aloud",
    "read-aloud-mcp"
  ]
}
```

To enable a still-optional feature, list it under `enabled`:

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

`disabled` wins if the same feature appears in both lists. `features.json` is
ignored by git so local choices do not leak into commits. Feature choices are
read during the install/build pipeline; if you change this file after an app
has already been generated, rerun the install/build step.

Packaged installs and updater rebuilds can use a persistent user override at
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/linux-features.json` with the same
shape. Checkout builds intentionally ignore that persistent user file when the
repo has a `.git` directory or worktree pointer, so packaged-install preferences
do not silently change local development builds or tests. For one-off builds,
set `CODEX_LINUX_FEATURES_CONFIG=/path/to/file.json` to point at an explicit
config file.
Native packages preserve the enabled feature id list in the packaged
update-builder bundle, so `codex-app-updater` rebuilds keep the same feature
choices across auto-updates.

You can also let the guided native setup helper discover feature manifests and
write `features.json`:

```bash
make setup-native

# non-interactive feature edits:
CODEX_BOOTSTRAP_NONINTERACTIVE=1 \
CODEX_LINUX_FEATURES=remote-mobile-control,read-aloud \
CODEX_LINUX_DISABLE_FEATURES=conversation-mode \
make setup-native
```

Disabling a feature in `features.json` only affects the next rebuild. The helper
does not delete local device keys, Read Aloud model files, plugin caches, Python
runtimes, or ydotool services. Feature-owned cleanup is a separate interactive
action:

```bash
CODEX_BOOTSTRAP_CLEANUP_FEATURES=remote-mobile-control,read-aloud make setup-native
```

The helper lists exact paths and deletes only paths confirmed with
`DELETE <exact path>`. Add `CODEX_BOOTSTRAP_DRY_RUN=1` to preview cleanup
targets without deleting them.

Each feature directory should include:

- `feature.json` — metadata, optional `defaultEnabled`, and entrypoints
- `README.md` — what it does, how to test it, and known risks
- optional `patch.js` — exports `applyMainBundlePatch(source, context)`, or
  descriptor patches when `feature.json` uses `entrypoints.patchDescriptors`
- optional `stage.sh` — install/build staging hook
- optional `test.js` — self-contained tests for the feature

`stage.sh` hooks run with `SCRIPT_DIR`, `INSTALL_DIR`, `WORK_DIR`, `ARCH`, and
`CODEX_OFFICIAL_APP_DIR` in the environment. `CODEX_UPSTREAM_APP_DIR` remains a
legacy alias for existing hooks.

Descriptor patches use the same shape as `scripts/patches/core/**/patch.js`.
They can target `main-bundle`, `webview-asset`, or `extracted-app` phases.
Feature descriptor ids are namespaced as `feature:<feature-id>:<descriptor-id>`
in patch reports and are optional by default.

Feature self-tests live inside each feature directory. Run them with:

```bash
node --test linux-features/*/test.js
```

Core Linux compatibility patches should stay in `scripts/patches/` until they
are deliberately migrated. Use `linux-features/` for configurable integrations
whose default can be changed without moving code into the core patch registry.
