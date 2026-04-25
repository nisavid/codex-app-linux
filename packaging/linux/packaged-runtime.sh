#!/bin/bash

codex_packaged_runtime_prelaunch() {
    if ! command -v systemctl >/dev/null 2>&1; then
        return 0
    fi

    if [ -z "${XDG_RUNTIME_DIR:-}" ] || [ ! -d "$XDG_RUNTIME_DIR" ]; then
        return 0
    fi

    if ! systemctl --user show-environment >/dev/null 2>&1; then
        return 0
    fi

    systemctl --user import-environment \
        DISPLAY \
        WAYLAND_DISPLAY \
        DBUS_SESSION_BUS_ADDRESS \
        XAUTHORITY \
        XDG_RUNTIME_DIR >/dev/null 2>&1 || true

    if command -v dbus-update-activation-environment >/dev/null 2>&1; then
        dbus-update-activation-environment --systemd \
            DISPLAY \
            WAYLAND_DISPLAY \
            DBUS_SESSION_BUS_ADDRESS \
            XAUTHORITY \
            XDG_RUNTIME_DIR >/dev/null 2>&1 || true
    fi

    systemctl --user disable --now codex-update-manager.service >/dev/null 2>&1 || true

    if systemctl --user is-enabled codex-app-updater.service >/dev/null 2>&1; then
        systemctl --user restart codex-app-updater.service >/dev/null 2>&1 || true
    else
        systemctl --user enable --now codex-app-updater.service >/dev/null 2>&1 || true
    fi
}

codex_packaged_runtime_export_env() {
    export CHROME_DESKTOP="codex-app.desktop"
    export BAMF_DESKTOP_FILE_HINT="/usr/share/applications/codex-app.desktop"
}
