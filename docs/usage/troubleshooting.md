# Troubleshooting

This guide lists the fastest checks for common Codex Desktop for Linux launch,
package, CLI, and updater problems.

## Start With Logs

Launcher log:

```bash
sed -n '1,160p' ~/.cache/codex-desktop/launcher.log
```

Updater service log:

```bash
sed -n '1,160p' ~/.local/state/codex-update-manager/service.log
```

Updater state:

```bash
sed -n '1,160p' ~/.local/state/codex-update-manager/state.json
codex-update-manager status --json
```

The updater uses these XDG paths:

```text
~/.config/codex-update-manager/config.toml
~/.local/state/codex-update-manager/state.json
~/.local/state/codex-update-manager/service.log
~/.cache/codex-update-manager/
```

The Electron launcher also writes:

```text
~/.cache/codex-desktop/launcher.log
~/.local/state/codex-desktop/app.pid
```

The PID file lets the updater wait until Electron exits before installing a
pending package.

## Symptoms

| Problem | What to try |
| --- | --- |
| `Error: write EPIPE` | Run `./codex-app/start.sh` directly instead of piping output. |
| Blank window | Check whether port `5175` is already in use: `ss -tlnp \| grep 5175`. |
| `ERR_CONNECTION_REFUSED` on `:5175` | Confirm `python3` works and port `5175` is free. The launcher serves the extracted webview bundle locally before Electron starts. |
| Stuck on the Codex logo splash | Check `~/.cache/codex-desktop/launcher.log`. Another process may be serving port `5175`, or `content/webview/` may be incomplete. |
| `CODEX_CLI_PATH` error | Install the CLI with `npm i -g @openai/codex` or `npm i -g --prefix ~/.local @openai/codex`. |
| Electron hangs while the CLI is outdated | Re-run the launcher, then inspect `~/.cache/codex-desktop/launcher.log` and `~/.local/state/codex-update-manager/service.log`. The CLI preflight is best-effort, uses a 1-hour registry lookup cooldown, falls back to `npm install -g --prefix ~/.local` when global install fails, and warns instead of blocking when automatic refresh fails. |
| GPU, Vulkan, or Wayland errors | The launcher sets `--ozone-platform-hint=auto`, `--disable-gpu-sandbox`, `--disable-gpu-compositing`, and `--enable-features=WaylandWindowDecorations` by default. To force X11, try `./codex-app/start.sh --ozone-platform=x11`. |
| Window flickering | GPU compositing is disabled by default. If flickering persists, try `./codex-app/start.sh --disable-gpu`. |
| Sandbox errors | The launcher already sets `--no-sandbox`. |
| Stale install or cached DMG | Run `./install.sh --fresh` to remove the generated app tree and redownload the DMG. |
| Usage help | Run `./install.sh --help` or `./codex-app/start.sh --help`. |
| `codex-update-manager` keeps running after package removal | Run `systemctl --user disable --now codex-update-manager.service`, then confirm `/opt/codex-desktop` is gone. |

## Webview Startup Checks

Codex Desktop expects the extracted webview assets to be available from a local
origin on port `5175`. The launcher starts `python3 -m http.server 5175` from
`content/webview/`, waits for the port, and checks that
`http://127.0.0.1:5175/index.html` contains expected Codex startup markers.

If the app opens to a blank window or never leaves the splash screen:

```bash
ss -tlnp | grep 5175
sed -n '1,200p' ~/.cache/codex-desktop/launcher.log
```

Port collisions and incomplete extracted assets should now fail fast in the
launcher log instead of hanging silently.

## Updater Recovery Notes

`codex-update-manager` stays unprivileged until it installs the rebuilt package.
The final installation uses:

- `codex-update-manager install-deb --path <package>`;
- `codex-update-manager install-rpm --path <package>`;
- `codex-update-manager install-pacman --path <package>`.

If a privileged install fails or is dismissed, the updater records `failed` and
does not reprompt every few seconds. If an `installing` state is interrupted by
a crash or restart, the daemon recovers it on the next run: already-installed
candidates become `installed`, existing package artifacts return to
`ready_to_install`, and missing artifacts become `failed`.

On Arch Linux, the final updater install step uses `pacman -U --noconfirm`
against the locally rebuilt `.pkg.tar.zst`; it does not update by running
`git pull`.
