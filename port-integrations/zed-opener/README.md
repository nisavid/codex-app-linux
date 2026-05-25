# Zed Opener

Adds Zed as an opt-in Linux editor opener in Codex App. The patch extends
the official app's Zed opener block with a Linux platform entry and reuses the
official app's `path:line:column` argument builder.

This integration is opt-in. The loader reads enabled integration ids from the root
config at `port-integrations/integrations.json`, then loads this integration's manifest
from `port-integrations/zed-opener/integration.json`.

To enable it locally, create the root config if needed:

```bash
cp port-integrations/integrations.example.json port-integrations/integrations.json
```

Then list `zed-opener` in `port-integrations/integrations.json`:

```json
{
  "enabled": [
    "zed-opener"
  ]
}
```

The Linux opener detects these commands in `PATH`, in order:

- `zed`
- `zeditor`
- `zedit`
- `zed-cli`

Run the integration tests with:

```bash
node --test port-integrations/zed-opener/test.js
```

To validate it against an extracted app bundle, enable `zed-opener` in an
integration config and run:

```bash
node scripts/patch-linux-window-ui.js /path/to/extracted/app.asar
```

Known risk: the patch depends on the official app's minified Zed opener block.
If that block changes shape, the integration fails soft and leaves the bundle
unchanged.
