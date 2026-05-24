# Open Target Discovery

Default Linux open-target integration for Codex App.

This feature augments the official OpenAI app bundle's Open menus with:

- a Terminal target discovered from `xdg-terminal-exec`, common terminal
  commands, or `.desktop` entries marked as terminal emulators
- Linux IDE/editor targets from known command-line launchers and dynamic
  `.desktop` discovery, including XDG, Flatpak, Snap, and JetBrains
  Toolbox-style entries
- a richer File Manager target that prefers installed file managers and can
  reveal files in Dolphin or Nautilus before falling back to Electron
  `shell.openPath`

The feature is enabled by default in this fork. It reads the current user's
desktop app entries and `PATH`, then launches the selected target as the same
user. It does not run commands through a shell, and it strips app-internal
Electron, Node, Codex, and wrapper environment variables before launching
desktop targets.

Disable it for checkout builds by copying
`linux-features/features.example.json` to `linux-features/features.json` and
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
the changed feature list.

For packaged installs and updater rebuilds, put the same JSON shape at
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/linux-features.json`. Checkout
builds ignore that persistent user file when the repo has a `.git` directory or
worktree pointer, so use `linux-features/features.json` or
`CODEX_LINUX_FEATURES_CONFIG` for development builds. For one-off builds, set
`CODEX_LINUX_FEATURES_CONFIG=/path/to/file.json`.

This feature is broader than `zed-opener`. If both are enabled, `zed-opener`
can provide the focused Zed target while this feature avoids adding a second
built-in Zed target and still discovers other editors.

Run the feature tests with:

```bash
node --test linux-features/open-target-discovery/test.js
```
