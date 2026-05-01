#!/usr/bin/env bash
set -euo pipefail

XDG_DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
XDG_STATE_HOME="${XDG_STATE_HOME:-${HOME}/.local/state}"

INSTALL_ROOT="${CODEX_USER_INSTALL_ROOT:-${XDG_DATA_HOME}/codex-app}"
APP_DIR="${INSTALL_ROOT}/app"
DMG_FILE="${INSTALL_ROOT}/Codex.dmg"
DMG_URL="https://persistent.oaistatic.com/codex-app-prod/Codex.dmg"

STATE_DIR="${XDG_STATE_HOME}/codex-app"
LOG_DIR="${STATE_DIR}/logs"
METADATA_FILE="${STATE_DIR}/metadata.env"
INSTALL_CONFIG_FILE="${STATE_DIR}/install.env"
ICON_PATH="${XDG_DATA_HOME}/icons/hicolor/512x512/apps/codex-app.png"
DESKTOP_FILE="${XDG_DATA_HOME}/applications/codex-app.desktop"

REPO_DIR_DEFAULT="${HOME}/workspace/codex-app-linux"
REPO_DIR="$REPO_DIR_DEFAULT"

ensure_layout() {
    mkdir -p "$STATE_DIR" "$LOG_DIR" "$(dirname "$ICON_PATH")" "$(dirname "$DESKTOP_FILE")"
}

load_install_config() {
    if [ -f "$INSTALL_CONFIG_FILE" ]; then
        # shellcheck disable=SC1090
        source "$INSTALL_CONFIG_FILE"
    fi
    REPO_DIR="${REPO_DIR:-$REPO_DIR_DEFAULT}"
}

load_metadata() {
    if [ -f "$METADATA_FILE" ]; then
        # shellcheck disable=SC1090
        source "$METADATA_FILE"
    fi
}

write_kv() {
    printf '%s=%q\n' "$1" "${2-}"
}

current_repo_head() {
    git -C "$REPO_DIR" rev-parse HEAD
}

remote_repo_head() {
    git -C "$REPO_DIR" ls-remote origin HEAD | awk 'NR==1 { print $1 }'
}

remote_dmg_headers() {
    curl -fsSIL "$DMG_URL" | tr -d '\r'
}

header_value() {
    local headers="$1"
    local name="$2"
    printf '%s\n' "$headers" | awk -F': ' -v target="$name" 'tolower($1) == tolower(target) { print $2; exit }'
}

extract_icon() {
    ensure_layout
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' RETURN

    7z e -y "$DMG_FILE" "Codex Installer/Codex.app/Contents/Resources/electron.icns" "-o${tmp_dir}" >/dev/null
    python3 - "$tmp_dir/electron.icns" "$ICON_PATH" <<'PY'
from PIL import Image
import sys

source_path, target_path = sys.argv[1], sys.argv[2]
with Image.open(source_path) as img:
    img.load()
    img.thumbnail((512, 512))
    img.save(target_path, format="PNG")
PY
}

record_metadata() {
    ensure_layout
    load_install_config

    local repo_head dmg_sha256 dmg_size electron_version dmg_headers dmg_etag dmg_last_modified dmg_content_length build_time repo_origin
    if [ -d "$REPO_DIR/.git" ]; then
        repo_head="$(current_repo_head)"
        repo_origin="$(git -C "$REPO_DIR" remote get-url origin)"
    else
        repo_head="unavailable"
        repo_origin="unavailable"
    fi
    dmg_sha256="$(sha256sum "$DMG_FILE" | awk '{ print $1 }')"
    dmg_size="$(stat -c '%s' "$DMG_FILE")"
    electron_version="$(cat "$APP_DIR/version")"
    build_time="$(date -Iseconds)"

    dmg_headers="$(remote_dmg_headers 2>/dev/null || true)"
    dmg_etag="$(header_value "$dmg_headers" "etag")"
    dmg_last_modified="$(header_value "$dmg_headers" "last-modified")"
    dmg_content_length="$(header_value "$dmg_headers" "content-length")"

    {
        write_kv BUILD_TIME "$build_time"
        write_kv REPO_ORIGIN "$repo_origin"
        write_kv REPO_HEAD "$repo_head"
        write_kv DMG_SHA256 "$dmg_sha256"
        write_kv DMG_SIZE "$dmg_size"
        write_kv DMG_ETAG "$dmg_etag"
        write_kv DMG_LAST_MODIFIED "$dmg_last_modified"
        write_kv DMG_CONTENT_LENGTH "$dmg_content_length"
        write_kv ELECTRON_VERSION "$electron_version"
        write_kv APP_DIR "$APP_DIR"
        write_kv ICON_PATH "$ICON_PATH"
        write_kv INSTALL_ROOT "$INSTALL_ROOT"
        write_kv REPO_DIR "$REPO_DIR"
    } > "$METADATA_FILE"
}
