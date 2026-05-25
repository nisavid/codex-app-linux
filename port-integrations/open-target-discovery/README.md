# Open Target Discovery

Default port integration for Codex App open-target integration.

This integration augments the official OpenAI app bundle's Open menus with:

- a Terminal target discovered from `xdg-terminal-exec`, common terminal
  commands, or `.desktop` entries marked as terminal emulators
- Linux IDE/editor targets from known command-line launchers and dynamic
  `.desktop` discovery, including XDG, Flatpak, Snap, and JetBrains
  Toolbox-style entries
- a richer File Manager target that prefers installed file managers and can
  reveal files in Dolphin or Nautilus before falling back to Electron
  `shell.openPath`

The integration is enabled by default in this fork. It reads the current user's
desktop app entries and `PATH`, then launches the selected target as the same
user. It does not run commands through a shell, and it strips app-internal
Electron, Node, Codex, and wrapper environment variables before launching
desktop targets.

Disable it for checkout builds by copying
`port-integrations/integrations.example.json` to `port-integrations/integrations.json` and
listing:

```json
{
  "enabled": [],
  "disabled": [
    "open-target-discovery"
  ]
}
```

Then rerun `./install.sh` or rebuild the package so the generated app picks up
the changed integration list.

For packaged installs and updater rebuilds, put the same JSON shape at
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/port-integrations.json`. Checkout
builds ignore that persistent user file when the repo has a `.git` directory or
worktree pointer, so use `port-integrations/integrations.json` or
`CODEX_PORT_INTEGRATIONS_CONFIG` for development builds. For one-off builds, set
`CODEX_PORT_INTEGRATIONS_CONFIG=/path/to/file.json`.

This integration is broader than `zed-opener`. If both are enabled, `zed-opener`
can provide the focused Zed target while this integration avoids adding a second
built-in Zed target and still discovers other editors.

Run the integration tests with:

```bash
node --test port-integrations/open-target-discovery/test.js
```
