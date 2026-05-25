# Copilot Reasoning Effort Defaults

This optional port integration patches Codex webview bundles so Copilot-auth
sessions can persist and select reasoning effort defaults for new chats.

By default, official OpenAI app bundle Copilot-auth paths only read and write
`copilot-default-model`, hardcode the loaded reasoning effort to `medium`, and
collapse Copilot model reasoning effort choices to one `medium` entry. This
integration keeps those changes local and opt-in instead of shipping them as a core
Linux compatibility patch.

Enable it by copying `port-integrations/integrations.example.json` to
`port-integrations/integrations.json` and adding the integration id:

```json
{
  "enabled": [
    "copilot-reasoning-effort"
  ]
}
```

Then rerun the install or package build so the ASAR patch step can apply the
integration to the generated app.

## What It Patches

- `use-model-settings-*.js` reads and writes
  `copilot-default-reasoning-effort` next to `copilot-default-model`.
- `font-settings-*.js` keeps the model's full `supportedReasoningEfforts` list
  for Copilot auth instead of forcing only `medium`.
- `index-*.js` keeps reasoning effort dropdown entries and the `/reasoning`
  command enabled when the normal model and effort prerequisites are present.

## Validation

Run the integration tests with:

```bash
node --test port-integrations/copilot-reasoning-effort/test.js
```

Or run all integration tests with:

```bash
node --test port-integrations/*/test.js
```

The patch is fail-soft. If the official OpenAI app bundle's minified bundle
shape changes, the build logs a warning and leaves the affected bundle
unchanged.
