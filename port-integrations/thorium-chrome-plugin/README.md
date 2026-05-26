# Thorium Chrome Plugin Support

This optional port integration extends the bundled Chrome plugin to recognize
Thorium as a Chromium-family browser.

It is disabled by default because Thorium is a narrower browser variant that the
core Linux port does not regularly test. For checkout builds, enable it by
adding the integration id to `port-integrations/integrations.json`. For
installed updater rebuilds, use
`${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/port-integrations.json` instead:

```json
{
  "enabled": [
    "thorium-chrome-plugin"
  ],
  "disabled": []
}
```

When enabled, the integration:

- adds Thorium native-messaging manifest locations for the generated launcher
- patches the staged Chrome plugin scripts to detect Thorium installs, profiles,
  running processes, default-browser desktop IDs, and launch commands
- adds Thorium to the Electron-side Chrome extension settings/status helper

Run the focused tests with:

```bash
node --test port-integrations/thorium-chrome-plugin/test.js
```
