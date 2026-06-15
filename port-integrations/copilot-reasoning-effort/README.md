# Copilot Reasoning Effort Defaults

This default-enabled port integration patches Codex webview bundles so Copilot-auth
sessions can persist and select reasoning effort defaults for new chats.

By default, official OpenAI app bundle Copilot-auth paths only read and write
`copilot-default-model`, hardcode the loaded reasoning effort to `medium`, and
collapse Copilot model reasoning effort choices to one `medium` entry. This
integration keeps those changes local to this fork instead of shipping them as a
core Linux compatibility patch.

Disable it by copying `port-integrations/integrations.example.json` to
`port-integrations/integrations.json` and adding the integration id:

```json
{
  "disabled": [
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

## Security Boundary

This integration changes the client-side model settings and preference surface.
OpenAI-hosted Copilot request handling remains authoritative for account
entitlement, quota, and request normalization. Backend behavior for non-medium
Copilot reasoning efforts is tracked in
[#100](https://github.com/nisavid/codex-app-linux/issues/100).

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
