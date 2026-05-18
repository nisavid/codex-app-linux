#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FILES_DIR="${SCRIPT_DIR}/files"
SCRIPT_REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SOURCE_REPO_ROOT="${CODEX_USER_LOCAL_SOURCE_REPO_DIR:-$SCRIPT_REPO_ROOT}"
XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-${HOME}/.config}"
XDG_STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"
INSTALL_ROOT="${CODEX_USER_INSTALL_ROOT:-${XDG_DATA_HOME}/codex-app}"
APP_BIN_DIR="${INSTALL_ROOT}/bin"
APP_LIB_DIR="${INSTALL_ROOT}/lib"
USER_BIN_DIR="${HOME}/.local/bin"
CONFIG_DIR="${XDG_CONFIG_HOME}/codex-app"
USER_LOCAL_ENV_FILE="${CONFIG_DIR}/user-local.env"
MANAGED_REPO_DIR="${INSTALL_ROOT}/managed-repo"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/codex-app"
FROM_UPDATE=0
ENABLE_TIMER=0
USER_LOCAL_OZONE_PLATFORM_SETTING=""

while [ $# -gt 0 ]; do
    case "$1" in
        --from-update)
            FROM_UPDATE=1
            ;;
        --enable-timer)
            ENABLE_TIMER=1
            ;;
        --force-x11|--x11-fallback)
            USER_LOCAL_OZONE_PLATFORM_SETTING="x11"
            ;;
        --no-force-x11|--no-x11-fallback)
            USER_LOCAL_OZONE_PLATFORM_SETTING="auto"
            ;;
        *)
            echo "Unknown option: $1" >&2
            exit 2
            ;;
    esac
    shift
done

copy_file() {
    local src="$1"
    local dst="$2"
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
}

write_user_local_preferences() {
    [ -n "$USER_LOCAL_OZONE_PLATFORM_SETTING" ] || return 0

    mkdir -p "$CONFIG_DIR"
    cat > "$USER_LOCAL_ENV_FILE" <<EOF
CODEX_USER_LOCAL_OZONE_PLATFORM=$(printf '%q' "$USER_LOCAL_OZONE_PLATFORM_SETTING")
EOF
}

repo_origin_url() {
    if [ -d "${SOURCE_REPO_ROOT}/.git" ]; then
        git -C "$SOURCE_REPO_ROOT" remote get-url origin 2>/dev/null || true
    fi
}

detected_repo_default_branch() {
    local branch=""
    if [ -d "${SOURCE_REPO_ROOT}/.git" ]; then
        branch="$(git -C "$SOURCE_REPO_ROOT" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null || true)"
        branch="${branch#origin/}"
        if [ -z "$branch" ]; then
            branch="$(git -C "$SOURCE_REPO_ROOT" symbolic-ref --quiet --short HEAD 2>/dev/null || true)"
            if [ -n "$branch" ] && ! git -C "$SOURCE_REPO_ROOT" rev-parse --verify --quiet "refs/remotes/origin/$branch" >/dev/null; then
                branch=""
            fi
        fi
    fi
    printf '%s\n' "$branch"
}

install_manager_files() {
    local systemd_user_dir="${XDG_CONFIG_HOME}/systemd/user"
    mkdir -p "$APP_BIN_DIR" "$APP_LIB_DIR" "${XDG_DATA_HOME}/applications" "$USER_BIN_DIR" "$STATE_DIR" "$systemd_user_dir"

    copy_file "${FILES_DIR}/share/common.sh" "${APP_LIB_DIR}/common.sh"
    copy_file "${FILES_DIR}/.local/bin/codex-app" "${APP_BIN_DIR}/codex-app"
    copy_file "${FILES_DIR}/.local/bin/codex-app-check-update" "${APP_BIN_DIR}/codex-app-check-update"
    copy_file "${FILES_DIR}/.local/bin/codex-app-update" "${APP_BIN_DIR}/codex-app-update"
    copy_file "${FILES_DIR}/.local/bin/codex-app-version" "${APP_BIN_DIR}/codex-app-version"

    cat > "${USER_BIN_DIR}/codex-app" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
INSTALL_ROOT="${CODEX_USER_INSTALL_ROOT:-${XDG_DATA_HOME}/codex-app}"
exec "${INSTALL_ROOT}/bin/codex-app" "$@"
EOF
    cat > "${USER_BIN_DIR}/codex-app-check-update" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
INSTALL_ROOT="${CODEX_USER_INSTALL_ROOT:-${XDG_DATA_HOME}/codex-app}"
exec "${INSTALL_ROOT}/bin/codex-app-check-update" "$@"
EOF
    cat > "${USER_BIN_DIR}/codex-app-update" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
INSTALL_ROOT="${CODEX_USER_INSTALL_ROOT:-${XDG_DATA_HOME}/codex-app}"
exec "${INSTALL_ROOT}/bin/codex-app-update" "$@"
EOF
    cat > "${USER_BIN_DIR}/codex-app-version" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
INSTALL_ROOT="${CODEX_USER_INSTALL_ROOT:-${XDG_DATA_HOME}/codex-app}"
exec "${INSTALL_ROOT}/bin/codex-app-version" "$@"
EOF

    sed "s|@USER_BIN_DIR@|${USER_BIN_DIR}|g" "${FILES_DIR}/.local/share/applications/codex-app.desktop" > "${XDG_DATA_HOME}/applications/codex-app.desktop"

    copy_file "${FILES_DIR}/.config/systemd/user/codex-app-update.service" "${systemd_user_dir}/codex-app-update.service"
    copy_file "${FILES_DIR}/.config/systemd/user/codex-app-update.timer" "${systemd_user_dir}/codex-app-update.timer"

    cat > "${STATE_DIR}/install.env" <<EOF
REPO_DIR=$(printf '%q' "$SOURCE_REPO_ROOT")
SOURCE_REPO_DIR=$(printf '%q' "$SOURCE_REPO_ROOT")
MANAGED_REPO_DIR=$(printf '%q' "$MANAGED_REPO_DIR")
REPO_ORIGIN_URL=$(printf '%q' "$(repo_origin_url)")
REPO_DEFAULT_BRANCH=$(printf '%q' "$(detected_repo_default_branch)")
INSTALL_ROOT=$(printf '%q' "$INSTALL_ROOT")
XDG_DATA_HOME=$(printf '%q' "$XDG_DATA_HOME")
EOF

    chmod +x \
        "${APP_BIN_DIR}/codex-app" \
        "${APP_BIN_DIR}/codex-app-check-update" \
        "${APP_BIN_DIR}/codex-app-update" \
        "${APP_BIN_DIR}/codex-app-version" \
        "${APP_LIB_DIR}/common.sh" \
        "${USER_BIN_DIR}/codex-app" \
        "${USER_BIN_DIR}/codex-app-check-update" \
        "${USER_BIN_DIR}/codex-app-update" \
        "${USER_BIN_DIR}/codex-app-version"
}

install_manager_files
write_user_local_preferences

if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    if [ "$ENABLE_TIMER" -eq 1 ]; then
        systemctl --user enable --now codex-app-update.timer >/dev/null 2>&1 || true
    fi
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${XDG_DATA_HOME}/applications" >/dev/null 2>&1 || true
fi

if [ "$FROM_UPDATE" -eq 0 ] && [ -x "${USER_BIN_DIR}/codex-app-update" ]; then
    "${USER_BIN_DIR}/codex-app-update" --record-only >/dev/null 2>&1 || true
fi

if [ "$FROM_UPDATE" -eq 0 ]; then
    echo "Installed user-local Codex desktop integration."
fi
