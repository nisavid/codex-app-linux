# Linux AppShots

`port-integrations/appshots` exposes the upstream AppShots composer entry on
Linux. It attaches the focused window screenshot plus best-effort AT-SPI text
to the composer.

This integration is enabled by default in this fork. Disable it before building
when the build should omit AppShots on Linux:

```json
{
  "disabled": [
    "appshots"
  ]
}
```

The integration is self-contained. It patches only the optional AppShots webview
availability gate, the Electron main-process AppShots handlers, and the
upstream AppShots hotkey settings row. It does not add AppShots-specific code
to `computer-use-linux`, core patch modules, default patch flow, or packaged
runtime hooks.

For window metadata and AT-SPI text, the integration shells out to the bundled
Linux Computer Use backend's existing `windows` and `state` commands. For the
screenshot, it uses an available desktop screenshot CLI such as `grim`,
`spectacle`, `gnome-screenshot`, `maim`, `scrot`, or ImageMagick `import`, then
crops the image to the focused window bounds in Electron.

Privacy and correctness constraints:

- The integration may briefly create a full-screen temporary screenshot before
  cropping it to the focused window.
- Capture fails closed when no focused window or usable bounds are available.
- Capture fails closed when no screenshot tool is available or the crop does not
  intersect the captured image.
- Global hotkeys are disabled by default on Linux until the user chooses one in
  AppShots settings. The dropdown mirrors upstream's bare-modifier choices where
  they are practical on Linux (`Alt + Alt` and `Shift + Shift`) and keeps
  `Ctrl+Super+A` as a non-bare fallback.
- `Alt + Alt` and `Shift + Shift` are backed by a port integration helper,
  `bare-modifier-monitor`, staged into `resources/native/`. It requires
  the left and right modifier keycodes, so tapping only one physical modifier
  twice does not trigger AppShots. It uses `xinput` and `xmodmap`, so it is
  expected to work on X11 sessions and fail closed elsewhere.

Run the integration self-test:

```bash
node --test port-integrations/appshots/test.js
```

To test in the app, rebuild the dev app, open a chat, open the composer
attachment/context menu, and use the AppShot entry.
