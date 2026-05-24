# Remote Control UI

Default-on Linux UI patches for the official OpenAI app bundle's
`remote_control` and Codex mobile surfaces.

This feature only opens the Linux UI gates. It does not fake backend state such
as connected clients, MFA completion, or remote control environments.

Disable it locally with:

```json
{
  "disabled": [
    "remote-control-ui"
  ]
}
```

Run the feature tests with:

```bash
node --test linux-features/remote-control-ui/test.js
```
