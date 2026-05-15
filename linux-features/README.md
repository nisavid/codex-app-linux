# Linux Features

`linux-features/` contains Linux integration modules for this wrapper. These
are not upstream Codex plugins; they are Linux-side extensions that can add
ASAR patches, staged resources, or build/install hooks.

## Defaults And Local Overrides

This fork enables `open-target-discovery` by default because it improves the
Linux Open menus with terminal, editor, and file-manager targets. It reads the
current user's desktop app entries and launches selected targets as that same
user; see
[`open-target-discovery/README.md`](open-target-discovery/README.md) for the
scope and trust notes.

To disable a default feature for a checkout build, copy `features.example.json`
to the git-ignored `features.json`, add the feature id under `disabled`, then
rerun `./install.sh` or the package build:

```json
{
  "enabled": [],
  "disabled": [
    "open-target-discovery"
  ]
}
```

To enable an optional feature, list it under `enabled`:

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
shape. For one-off builds, set `CODEX_LINUX_FEATURES_CONFIG=/path/to/file.json`
to point at an explicit config file.

Each feature directory should include:

- `feature.json` — metadata, optional `defaultEnabled`, and entrypoints
- `README.md` — what it does, how to test it, and known risks
- optional `patch.js` — exports `applyMainBundlePatch(source, context)`, or
  descriptor patches when `feature.json` uses `entrypoints.patchDescriptors`
- optional `stage.sh` — install/build staging hook
- optional `test.js` — self-contained tests for the feature

`stage.sh` hooks run with `SCRIPT_DIR`, `INSTALL_DIR`, `WORK_DIR`, `ARCH`, and
`CODEX_UPSTREAM_APP_DIR` in the environment.

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
