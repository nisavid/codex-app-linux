#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

APP_DIR="${APP_DIR:-$PWD/codex-app}"
DIST_DIR="${DIST_DIR:-$PWD/dist}"
DMG_PATH="${DMG:-$PWD/Codex.dmg}"
CHECKSUM_FILE="${CHECKSUM_FILE:-$DIST_DIR/SHA256SUMS}"
REQUIRE_RELEASE_SIGNATURE="${REQUIRE_RELEASE_SIGNATURE:-0}"
CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA="${CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA:-0}"

info() {
    echo "[release-gate] $*" >&2
}

error() {
    echo "[release-gate][ERROR] $*" >&2
    exit 1
}

require_file() {
    local path="$1"
    local label="$2"
    [ -f "$path" ] || error "Missing $label: $path"
}

require_dir() {
    local path="$1"
    local label="$2"
    [ -d "$path" ] || error "Missing $label: $path"
}

sri_to_hex() {
    local sri="$1"
    local payload="${sri#sha256-}"
    printf '%s' "$payload" | base64 -d 2>/dev/null | od -An -tx1 | tr -d ' \n'
}

expected_dmg_sha256() {
    if [ -n "${CODEX_DMG_SHA256:-}" ]; then
        printf '%s\n' "$CODEX_DMG_SHA256"
        return
    fi

    local sri="${CODEX_DMG_SRI:-}"
    if [ -z "$sri" ] && [ -f "$REPO_DIR/flake.nix" ]; then
        sri="$(grep -oP 'hash = "\Ksha256-[^"]+' "$REPO_DIR/flake.nix" | head -n 1 || true)"
    fi

    [ -n "$sri" ] || error "Set CODEX_DMG_SHA256 or CODEX_DMG_SRI before releasing"
    sri_to_hex "$sri"
}

verify_dmg_hash() {
    require_file "$DMG_PATH" "upstream DMG"

    local expected actual
    expected="$(expected_dmg_sha256)"
    [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || error "Trusted DMG hash is not a hex SHA-256 digest"
    actual="$(sha256sum "$DMG_PATH" | awk '{print $1}')"

    [ "$actual" = "${expected,,}" ] || error "DMG hash mismatch: expected ${expected,,}, got $actual"
    info "Verified DMG SHA-256: $actual"
}

inspect_generated_app() {
    require_dir "$APP_DIR" "generated app"
    node "$REPO_DIR/scripts/inspect-electron-security.js" "$APP_DIR"
}

collect_packages() {
    shopt -s nullglob
    PACKAGES=(
        "$DIST_DIR"/codex-app_*.deb
        "$DIST_DIR"/codex-app-*.rpm
        "$DIST_DIR"/codex-app-*.pkg.tar.*
    )
    shopt -u nullglob
    [ "${#PACKAGES[@]}" -gt 0 ] || error "No native packages found in $DIST_DIR"
}

verify_package_metadata() {
    [ "$CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA" = "1" ] && return

    local package name
    for package in "${PACKAGES[@]}"; do
        case "$package" in
            *.deb)
                command -v dpkg-deb >/dev/null 2>&1 || error "dpkg-deb is required to inspect $package"
                name="$(dpkg-deb -f "$package" Package)"
                ;;
            *.rpm)
                command -v rpm >/dev/null 2>&1 || error "rpm is required to inspect $package"
                name="$(rpm -qp --queryformat '%{NAME}' "$package")"
                ;;
            *.pkg.tar.*)
                command -v pacman >/dev/null 2>&1 || error "pacman is required to inspect $package"
                name="$(pacman -Qp "$package" | awk '{print $1}')"
                ;;
            *)
                error "Unsupported package artifact: $package"
                ;;
        esac

        [ "$name" = "codex-app" ] || error "Package $package has unexpected name '$name'"
    done
}

write_checksums() {
    mkdir -p "$DIST_DIR"
    : > "$CHECKSUM_FILE"

    local package
    for package in "${PACKAGES[@]}"; do
        (
            cd "$DIST_DIR"
            sha256sum "$(basename "$package")"
        ) >> "$CHECKSUM_FILE"
    done
    info "Wrote $(realpath --relative-to="$PWD" "$CHECKSUM_FILE" 2>/dev/null || printf '%s' "$CHECKSUM_FILE")"
}

sign_checksums() {
    local signature="${CHECKSUM_FILE}.asc"

    if [ "$REQUIRE_RELEASE_SIGNATURE" != "1" ] && [ -z "${CODEX_RELEASE_GPG_KEY:-}" ]; then
        info "Skipping detached signature; set REQUIRE_RELEASE_SIGNATURE=1 and CODEX_RELEASE_GPG_KEY for public releases"
        return
    fi

    command -v gpg >/dev/null 2>&1 || error "gpg is required to sign release checksums"
    [ -n "${CODEX_RELEASE_GPG_KEY:-}" ] || error "CODEX_RELEASE_GPG_KEY is required when release signatures are required"

    gpg --batch --yes \
        --local-user "$CODEX_RELEASE_GPG_KEY" \
        --output "$signature" \
        --detach-sign \
        --armor \
        "$CHECKSUM_FILE"
    require_file "$signature" "release checksum signature"
    info "Wrote $(realpath --relative-to="$PWD" "$signature" 2>/dev/null || printf '%s' "$signature")"
}

main() {
    verify_dmg_hash
    inspect_generated_app
    collect_packages
    verify_package_metadata
    write_checksums
    sign_checksums
    info "Release gate passed"
}

main "$@"
