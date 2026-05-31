# Linux Integrations Architecture

`port-integrations/` is the extension boundary for optional Linux integrations.
Core keeps a small generic loader; integration-specific behavior lives in integration
directories and is disabled by default.

## Layout

Repository integrations live directly under `port-integrations/<integration-id>/`.

User-local integrations live under `port-integrations/local/<integration-id>/`. The
`port-integrations/local/` directory is ignored by git, so a user can keep private
or experimental integrations in the checkout without accidentally committing
them.

Every integration needs a `integration.json` manifest and a neighboring `README.md`.
The README is required for both repository integrations and git-ignored local
integrations, and should describe what the integration does, how to test it, and known
support risks.

```json
{
  "id": "my-integration",
  "title": "My Integration",
  "description": "Optional Linux integration.",
  "defaultEnabled": false
}
```

Integration ids must match `^[a-z0-9][a-z0-9-]*$`. Repository and local integrations
share one id namespace; local integrations cannot shadow repository integrations.
`defaultEnabled: true` is rejected. Enabling always happens through the
git-ignored `port-integrations/integrations.json` file:

```json
{
  "enabled": ["my-integration"]
}
```

## Lifecycle

The build pipeline loads enabled integrations in these phases:

1. ASAR patching: patch descriptors modify extracted upstream app files.
2. App staging: declarative resources and runtime hooks are copied into
   `codex-app/`.
3. Legacy staging: optional `stage.sh` hooks run for integrations that still need
   custom install-time logic.
4. Native packaging: optional package hooks can mutate the `.deb`, `.rpm`, or
   pacman staging root.
5. Runtime: the launcher consumes staged environment files, prelaunch hooks,
   Electron args, and cold-start hooks.

Native packages copy the configured integration root into the packaged
`update-builder` bundle, including `port-integrations/local/`, and write a
sanitized `integrations.json` containing only the enabled ids. Local auto-updates
therefore rebuild with the same opt-in integrations.

Declarative staged files are tracked in
`.codex-linux/port-integrations-staged.json`. On the next install, the framework
removes the previously tracked declarative resources and runtime hooks before
staging the currently enabled set, so disabling a integration removes its
framework-owned runtime hooks. Legacy `stage.sh` hooks are not tracked by this
manifest and must clean up any integration-owned files themselves.

## Manifest Keys

`entrypoints` keeps the existing patch and staging API:

```json
{
  "entrypoints": {
    "patchDescriptors": "./patch.js",
    "patches": "./patch.js",
    "mainBundlePatch": "./patch.js",
    "stageHook": "./stage.sh"
  }
}
```

Prefer `patchDescriptors` for new patches. Integration descriptor ids are reported
as `integration:<integration-id>:<descriptor-id>` and are optional in CI by default.
`mainBundlePatch` is the compatibility path for older integrations that export
`applyMainBundlePatch(source, context)`.

Use `requires` and `conflicts` to declare integration relationships:

```json
{
  "requires": ["read-aloud"],
  "conflicts": ["other-voice-loop"]
}
```

The setup wizard, installer, patcher, and package builders validate these
relationships before applying enabled integrations.

## Declarative App Staging

Use `resources` to copy files into the generated app directory:

```json
{
  "resources": [
    {
      "source": "assets/tool.json",
      "target": ".codex-linux/integrations/my-integration/tool.json",
      "mode": "0644"
    }
  ]
}
```

`source` stays inside the integration directory. `target` is relative to the app
directory and must point to a file or subdirectory, not the app root itself.
File modes are optional, but when present they must be quoted octal strings
such as `"0644"` or `"0755"`; numeric JSON modes are rejected. Declared modes
are recorded in the staged manifest and restored after native package
permission normalization, so restrictive resource modes survive `.deb`, `.rpm`,
and pacman packaging.

Use `runtimeHooks` for launcher-visible hooks:

```json
{
  "runtimeHooks": {
    "env": "env",
    "prelaunch": "prelaunch.sh",
    "electronArgs": "electron-args",
    "coldStart": "cold-start.sh",
    "afterExit": "after-exit.sh"
  }
}
```

The runtime hook types map to:

- `env`: copied to `.codex-linux/env.d/`; each non-comment line is exported as
  literal `KEY=VALUE` with no shell evaluation.
- `prelaunch`: copied to `.codex-linux/prelaunch.d/`; executable hooks run
  synchronously before the packaged runtime prelaunch and webview setup.
- `electronArgs`: copied to `.codex-linux/electron-args.d/`; each non-comment
  line is appended as one Electron argument.
- `coldStart`: copied to `.codex-linux/cold-start.d/`; executable hooks run in
  the background during cold start, after bundled plugin cache sync.
- `afterExit`: copied to `.codex-linux/after-exit.d/`; executable hooks run
  after Electron exits. Hook failures are logged and the launcher preserves
  Electron's original exit status.

Runtime hooks receive `CODEX_HOME`, `CODEX_LINUX_APP_DIR`,
`CODEX_LINUX_APP_STATE_DIR`, `CODEX_LINUX_FEATURES_DIR`, and
`CODEX_LINUX_LAUNCHER_LOG`. Executable hooks also receive
`CODEX_LINUX_FEATURE_HOOK_PHASE`; `afterExit` additionally receives
`CODEX_LINUX_ELECTRON_EXIT_STATUS`. Use this pattern for user-home artifacts
such as Codex skills: stage the source file with `resources` under
`.codex-linux/integrations/<integration-id>/...`, then copy it from
`$CODEX_LINUX_FEATURES_DIR/<integration-id>/...` to `$CODEX_HOME/skills/...` in a
`runtimeHooks.prelaunch` script. Do not write user-home files from `stage.sh`;
install, package, and updater rebuilds may run outside the real user's session.

## Package Hooks

Use `packageHooks` only when a integration must mutate native package staging:

```json
{
  "packageHooks": [
    {
      "path": "package.sh",
      "formats": ["deb", "rpm", "pacman"]
    }
  ]
}
```

Hooks run with:

- `PACKAGE_FORMAT`
- `PACKAGE_NAME`
- `PACKAGE_VERSION`
- `PACKAGE_ROOT` / `PACKAGE_STAGING_ROOT`
- `APP_DIR` / `PACKAGE_APP_DIR`
- `REPO_DIR`

Package hooks should be idempotent and narrowly scoped.

## Local Integration Example

Create a private integration without touching tracked files:

```bash
mkdir -p port-integrations/local/my-integration
$EDITOR port-integrations/local/my-integration/integration.json
```

Then enable it:

```bash
cp port-integrations/integrations.example.json port-integrations/integrations.json
$EDITOR port-integrations/integrations.json
make install-native
```

`make setup-native` also discovers local integrations, marks them as `[local]`,
and can enable them by id or list number.

## Design Rule

If a change is required for the basic Linux app to launch and behave correctly
for most users, it belongs in core patches under `scripts/patches/`.

If a change is optional, distro-specific, editor-specific, browser-specific,
workflow-specific, or likely to add future support burden for a minority of
users, put it in `port-integrations/` and keep it disabled by default.
