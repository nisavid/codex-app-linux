# Troubleshooting

This guide lists the fastest checks for common Codex App for Linux launch,
package, CLI, and updater problems.

## Start With Logs

Launcher log:

```bash
sed -n '1,160p' ~/.cache/codex-app/launcher.log
```

Updater service log:

```bash
sed -n '1,160p' ~/.local/state/codex-app-updater/service.log
```

Updater state:

```bash
sed -n '1,160p' ~/.local/state/codex-app-updater/state.json
codex-app-updater status --json
```

The updater uses these XDG paths:

```text
~/.config/codex-app-updater/config.toml
~/.local/state/codex-app-updater/state.json
~/.local/state/codex-app-updater/service.log
~/.cache/codex-app-updater/
```

The Electron launcher also writes:

```text
~/.cache/codex-app/launcher.log
~/.local/state/codex-app/app.pid
```

The PID file lets the updater wait until Electron exits before installing a
pending package.

## Symptoms

| Problem | What to try |
| --- | --- |
| `Error: write EPIPE` | Run `./codex-app/start.sh` directly instead of piping output. |
| Blank window | Check whether port `5175` is already in use: `ss -tlnp \| grep 5175`. |
| `ERR_CONNECTION_REFUSED` on `:5175` | Confirm `python3` works and port `5175` is free. The launcher serves the extracted webview bundle locally before Electron starts. |
| `webview bundle is missing or empty` | Regenerate the app with `./install.sh` or `make build-app`; the generated app must contain `content/webview/index.html`. |
| Stuck on the Codex logo splash | Check `~/.cache/codex-app/launcher.log`. Another process may be serving port `5175`, or `content/webview/` may be incomplete or fail integrity validation. |
| `CODEX_CLI_PATH` error | Install the CLI with `npm i -g @openai/codex` or `npm i -g --prefix ~/.local @openai/codex`. If you intentionally use another install, set `CODEX_CLI_PATH=/path/to/codex` for one launch or add `cli_path = "/path/to/codex"` to `~/.config/codex-app-updater/config.toml`. |
| Electron hangs while the CLI is outdated | Re-run the launcher, then inspect `~/.cache/codex-app/launcher.log` and `~/.local/state/codex-app-updater/service.log`. The CLI preflight is best-effort, uses a 1-hour registry lookup cooldown, falls back to `npm install -g --prefix ~/.local` when global install fails, and warns instead of blocking when automatic refresh fails. |
| GPU, Vulkan, or Wayland errors | The launcher sets `--ozone-platform-hint=auto` by default and adds `--enable-features=WaylandWindowDecorations` only when `--ozone-platform=wayland` is selected. To force X11, try `./codex-app/start.sh --ozone-platform=x11`. |
| Window flickering | Try `CODEX_ELECTRON_DISABLE_GPU_COMPOSITING=1 ./codex-app/start.sh` to use the legacy compositing workaround. If flickering persists, try `./codex-app/start.sh --disable-gpu`. |
| Sandbox errors | The launcher keeps Electron sandboxing enabled by default. As a temporary compatibility fallback, run `CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 ./codex-app/start.sh` and treat that mode as lower security. |
| `gh auth status` works in a terminal but fails inside Codex App | The app shell may be using isolated XDG paths or missing keyring DBus access. See [GitHub CLI auth in app-launched shells](../github-cli-auth.md). |
| Rust installer or managed Node runtime fails on hardened hosts | If `/tmp` is mounted `noexec`, set `TMPDIR` and `XDG_CACHE_HOME` to executable user-owned directories before install/build commands. |
| `ConnectTimeoutError` or slow Electron downloads during `@electron/rebuild` | Retry `make build-app`. If the network path is consistently blocked, set `ELECTRON_MIRROR` for the Electron runtime and `ELECTRON_HEADERS_URL` for Electron headers. |
| Stale install or cached DMG | Run `./install.sh --fresh` to remove the generated app tree and redownload the DMG. |
| Usage help | Run `./install.sh --help` or `./codex-app/start.sh --help`. |
| Computer Use plugin invisible in UI | Confirm the UI patch is enabled: either build with `CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1`, or set `"codex-linux-computer-use-ui-enabled": true` in `${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/settings.json`, then remember account-side rollout can still hide official OpenAI app bundle UI paths. |
| Computer Use `doctor` reports `ydotool not running` | Start the distro-provided daemon (`ydotoold` or, on some Fedora releases, `ydotool.service`), then add your user to an input-capable group for `/dev/uinput` and the daemon socket. Common group names include `input`, `uinput`, `plugdev`, and `wheel`; check your distro. |
| Computer Use `doctor` reports `ydotool_socket: Permission denied` | Adjust the `ydotoold` service/socket so the desktop user can connect, commonly by making the socket group-readable by an input-capable group such as `input`, `uinput`, `plugdev`, or `wheel`. |
| Computer Use `doctor` reports `ydotool_socket: Protocol wrong type for socket` | The daemon socket may be a Unix datagram socket rather than a stream socket. Upgrade or rebuild to a backend with datagram-aware ydotool socket checks, then rerun `doctor`; this error does not by itself prove that `ydotoold` is absent or unusable. |
| Computer Use keyboard input produces the wrong characters | Check the active keyboard layout and key remaps. Raw key synthesis can be physical-keycode based, so non-QWERTY layouts can transform requested key names after the event reaches the compositor. Temporarily switch to a standard US/QWERTY layout to isolate layout effects. |
| Computer Use `type_text` or paste-style input does not insert text | Text insertion may depend on setting the clipboard and sending a paste shortcut. Custom layouts or remapped modifier keys can break that shortcut even when pointer input and window focus work. Try a standard US/QWERTY layout, or verify the clipboard and shortcut path separately. |
| Computer Use AT-SPI tree is empty or sparse | Run `./codex-app/resources/plugins/openai-bundled/plugins/computer-use/bin/codex-computer-use-linux setup` where supported, confirm toolkit accessibility is enabled, then restart the target app. Some non-GNOME sessions still use the historical `org.gnome.desktop.interface toolkit-accessibility` key. Also check that `NO_AT_BRIDGE=1` is not set in the target app's environment. Some apps expose limited AT-SPI nodes even when screenshot, focus, and pointer input paths are healthy. |
| `codex-app-updater` keeps running after package removal | Run `systemctl --user disable --now codex-app-updater.service`, then confirm `/opt/codex-app` is gone. |

## Webview Startup Checks

Codex expects the extracted webview assets to be available from a local
origin on port `5175`. The launcher starts
`python3 -m http.server --bind 127.0.0.1 5175` from `content/webview/`, waits
for the port, and checks that
`http://127.0.0.1:5175/index.html` contains expected Codex startup markers and
that the origin serves the startup assets recorded in
`.codex-linux/webview-integrity.sha256`. Only loopback access is expected.

If the app opens to a blank window or never leaves the splash screen:

```bash
ss -tlnp | grep 5175
curl -fsS http://127.0.0.1:5175/index.html | grep 'startup-loader'
sed -n '1,200p' ~/.cache/codex-app/launcher.log
```

Port collisions and incomplete extracted assets should now fail fast in the
launcher log instead of hanging silently.

## Updater Recovery Notes

`codex-app-updater status` reports `cli_path` and `cli_path_source`. The source
shows whether the selected CLI came from `CODEX_CLI_PATH`, updater config,
persisted updater state, launch `PATH`, or a known package-manager fallback
path. An explicit `--cli-path` can also be used with `codex-app-updater
cli-preflight`; later `status` output reports the current resolver source as
`env`, `config`, `persisted`, `path`, `known_path`, or `unknown` when no CLI was
found.

`codex-app-updater` stays unprivileged until it installs the rebuilt package.
The final installation uses:

- `codex-app-updater install-deb --path <package>`;
- `codex-app-updater install-rpm --path <package>`;
- `codex-app-updater install-pacman --path <package>`.

If a privileged install fails or is dismissed, the updater records `failed` and
does not reprompt every few seconds. If an `installing` state is interrupted by
a crash or restart, the daemon recovers it on the next run: already-installed
candidates become `installed`, existing package artifacts return to
`ready_to_install`, and missing artifacts become `failed`.

On Arch Linux, the final updater install step uses `pacman -U --noconfirm`
against the locally rebuilt `.pkg.tar.zst`; it does not update by running
`git pull`.
