# Frameless Titlebar

This optional port integration hides the Electron titlebar overlay controls and
removes the native menu chrome from the main Codex window. It is intended for
Wayland compositors or window managers where compositor-managed decorations
already provide the expected window controls, such as Hyprland setups.

The default build leaves the existing titlebar overlay behavior in place.
Enable this only when the built-in Codex titlebar/buttons visually conflict
with your desktop environment.

Enable it by copying `port-integrations.example.json` to
`port-integrations.json` and listing the port integration id:

```json
{
  "enabled": [
    "frameless-titlebar"
  ]
}
```

Then rerun `./install.sh` or the native package build flow so the ASAR patches
are regenerated with this port integration enabled.

## Testing

Run the port integration's unit tests from the repository root:

```bash
node --test port-integrations/frameless-titlebar/test.js
```

For a manual check, enable the integration as above, rebuild, and launch the app:

- The primary window should show no Electron-drawn titlebar overlay buttons
  (minimize/maximize/close in the top-right corner) and no menu bar.
- Window move, resize, and close/minimize/maximize should work through your
  compositor's bindings (for example Hyprland's `bindm` mouse binds and
  `killactive`/`fullscreen` dispatchers).
- Changing the system dark/light theme must not crash the app or repaint a
  titlebar strip; the patch removes all Linux `setTitleBarOverlay` calls,
  which would otherwise throw on a window created without `titleBarOverlay`.

## Known risks

This removes Codex's Electron-provided Linux titlebar buttons.
Window movement, resize, and close/minimize/maximize controls then depend on
your compositor or desktop environment.
