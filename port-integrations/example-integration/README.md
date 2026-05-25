# Example Port Integration

This is a disabled-by-default example that documents the `port-integrations`
contract. It is intentionally harmless and does not patch the real Codex bundle.

To try it locally, copy `port-integrations/integrations.example.json` to
`port-integrations/integrations.json` and add:

```json
{
  "enabled": [
    "example-integration"
  ]
}
```

The example `patch.js` replaces a synthetic marker used only in tests. The
example `stage.sh` is a no-op hook that prints a short message.
