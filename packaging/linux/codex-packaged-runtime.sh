#!/bin/bash

codex_packaged_runtime_prelaunch() {
    codex_packaged_runtime_prelaunch_background >/dev/null 2>&1 &
}

codex_packaged_runtime_prelaunch_background() {
    if [ "__CODEX_PACKAGE_ENABLE_UPDATER__" != "1" ]; then
        return 0
    fi

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
        if ! systemctl --user is-active codex-app-updater.service >/dev/null 2>&1; then
            systemctl --user start codex-app-updater.service >/dev/null 2>&1 || true
        fi
    else
        systemctl --user enable --now codex-app-updater.service >/dev/null 2>&1 || true
    fi

    codex_packaged_runtime_trigger_update_check
}

codex_packaged_runtime_trigger_update_check() {
    if ! command -v codex-app-updater >/dev/null 2>&1; then
        return 0
    fi

    if command -v systemd-run >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
        systemd-run --user \
            --unit=codex-app-updater-launch-check \
            --collect \
            --quiet \
            /usr/bin/codex-app-updater check-now --if-stale >/dev/null 2>&1 || true
        return 0
    fi

    codex-app-updater check-now --if-stale >/dev/null 2>&1 || true
}

codex_packaged_runtime_export_env() {
    export CHROME_DESKTOP="codex-app.desktop"
    export BAMF_DESKTOP_FILE_HINT="/usr/share/applications/codex-app.desktop"
}
