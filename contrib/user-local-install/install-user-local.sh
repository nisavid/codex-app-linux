#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FILES_DIR="${SCRIPT_DIR}/files"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
INSTALL_ROOT="${HOME}/.local/lib/codex-app"
PRIVATE_BIN_DIR="${INSTALL_ROOT}/bin"
PRIVATE_LIB_DIR="${INSTALL_ROOT}/lib"
STATE_DIR="${XDG_STATE_HOME:-${HOME}/.local/state}/codex-app"
FROM_UPDATE=0
ENABLE_TIMER=0

while [ $# -gt 0 ]; do
    case "$1" in
        --from-update)
            FROM_UPDATE=1
            ;;
        --enable-timer)
            ENABLE_TIMER=1
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

install_manager_files() {
    local systemd_user_dir="${XDG_CONFIG_HOME:-${HOME}/.config}/systemd/user"
    mkdir -p "$PRIVATE_BIN_DIR" "$PRIVATE_LIB_DIR" "${HOME}/.local/share/applications" "${HOME}/.local/bin" "$STATE_DIR" "$systemd_user_dir"

    copy_file "${FILES_DIR}/.local/lib/codex-app/common.sh" "${PRIVATE_LIB_DIR}/common.sh"
    copy_file "${FILES_DIR}/.local/bin/codex-app" "${PRIVATE_BIN_DIR}/codex-app"
    copy_file "${FILES_DIR}/.local/bin/codex-app-check-update" "${PRIVATE_BIN_DIR}/codex-app-check-update"
    copy_file "${FILES_DIR}/.local/bin/codex-app-update" "${PRIVATE_BIN_DIR}/codex-app-update"
    copy_file "${FILES_DIR}/.local/bin/codex-app-version" "${PRIVATE_BIN_DIR}/codex-app-version"

    cat > "${HOME}/.local/bin/codex-app" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exec "${HOME}/.local/lib/codex-app/bin/codex-app" "$@"
EOF
    cat > "${HOME}/.local/bin/codex-app-check-update" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exec "${HOME}/.local/lib/codex-app/bin/codex-app-check-update" "$@"
EOF
    cat > "${HOME}/.local/bin/codex-app-update" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exec "${HOME}/.local/lib/codex-app/bin/codex-app-update" "$@"
EOF
    cat > "${HOME}/.local/bin/codex-app-version" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
exec "${HOME}/.local/lib/codex-app/bin/codex-app-version" "$@"
EOF

    sed "s|@HOME@|${HOME}|g" "${FILES_DIR}/.local/share/applications/codex-app.desktop" > "${HOME}/.local/share/applications/codex-app.desktop"

    copy_file "${FILES_DIR}/.config/systemd/user/codex-app-update.service" "${systemd_user_dir}/codex-app-update.service"
    copy_file "${FILES_DIR}/.config/systemd/user/codex-app-update.timer" "${systemd_user_dir}/codex-app-update.timer"

    cat > "${STATE_DIR}/install.env" <<EOF
REPO_DIR=$(printf '%q' "$REPO_ROOT")
INSTALL_ROOT=$(printf '%q' "$INSTALL_ROOT")
EOF

    chmod +x \
        "${PRIVATE_BIN_DIR}/codex-app" \
        "${PRIVATE_BIN_DIR}/codex-app-check-update" \
        "${PRIVATE_BIN_DIR}/codex-app-update" \
        "${PRIVATE_BIN_DIR}/codex-app-version" \
        "${PRIVATE_LIB_DIR}/common.sh" \
        "${HOME}/.local/bin/codex-app" \
        "${HOME}/.local/bin/codex-app-check-update" \
        "${HOME}/.local/bin/codex-app-update" \
        "${HOME}/.local/bin/codex-app-version"
}

install_manager_files

if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload >/dev/null 2>&1 || true
    if [ "$ENABLE_TIMER" -eq 1 ]; then
        systemctl --user enable --now codex-app-update.timer >/dev/null 2>&1 || true
    fi
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true
fi

if [ -x "${HOME}/.local/bin/codex-app-update" ]; then
    "${HOME}/.local/bin/codex-app-update" --record-only >/dev/null 2>&1 || true
fi

if [ "$FROM_UPDATE" -eq 0 ]; then
    echo "Installed user-local Codex app integration."
fi
