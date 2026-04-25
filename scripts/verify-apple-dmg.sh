#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DMG_PATH="${DMG:-$PWD/Codex.dmg}"
EXPECTED_BUNDLE_ID="com.openai.codex"
EXPECTED_TEAM_ID="2DC432GLL2"
EXPECTED_DEVELOPER_ID="Developer ID Application: OpenAI OpCo, LLC (2DC432GLL2)"
EXPECTED_SPARKLE_KEY="rhcBvttuqDFriyNqwTQJR3L4UT1WjIK4QxtwtwusVic="
REQUIRE_DMG_GATEKEEPER="${CODEX_REQUIRE_DMG_GATEKEEPER:-0}"
REQUIRE_DMG_STAPLE="${CODEX_REQUIRE_DMG_STAPLE:-0}"
MOUNT_DIR=""

usage() {
    cat <<'EOF'
Usage: verify-apple-dmg.sh [--dmg PATH]

Runs macOS trust checks for the upstream Codex DMG and contained Codex.app.
The app bundle checks are required. DMG Gatekeeper/staple checks are recorded
by default and can be made fatal with CODEX_REQUIRE_DMG_GATEKEEPER=1 and
CODEX_REQUIRE_DMG_STAPLE=1.

Environment:
  CODEX_DMG_SHA256                       Expected DMG SHA-256 hex digest
  CODEX_DMG_SRI                          Expected DMG sha256-... SRI digest
  CODEX_REQUIRE_DMG_GATEKEEPER=1         Fail when DMG Gatekeeper assessment fails
  CODEX_REQUIRE_DMG_STAPLE=1             Fail when DMG stapler validation fails
EOF
}

info() {
    echo "[apple-dmg-verify] $*" >&2
}

warn() {
    echo "[apple-dmg-verify][WARN] $*" >&2
}

error() {
    echo "[apple-dmg-verify][ERROR] $*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || error "$1 is required; run this script on macOS with Xcode command line tools"
}

require_file() {
    [ -f "$1" ] || error "Missing file: $1"
}

cleanup() {
    if [ -n "$MOUNT_DIR" ] && [ -d "$MOUNT_DIR" ]; then
        hdiutil detach "$MOUNT_DIR" -quiet >/dev/null 2>&1 || true
        rmdir "$MOUNT_DIR" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

sri_to_hex() {
    local sri="$1"
    local payload="${sri#sha256-}"
    printf '%s' "$payload" | base64 -D 2>/dev/null | od -An -tx1 | tr -d ' \n'
}

flake_dmg_sri() {
    awk '
        /Codex\.dmg/ { found_dmg = 1 }
        found_dmg && /hash = "sha256-/ { print; exit }
    ' "$REPO_DIR/flake.nix" | sed -E 's/.*(sha256-[^"]+).*/\1/'
}

expected_dmg_sha256() {
    if [ -n "${CODEX_DMG_SHA256:-}" ]; then
        printf '%s\n' "$CODEX_DMG_SHA256"
        return
    fi

    local sri="${CODEX_DMG_SRI:-}"
    if [ -z "$sri" ] && [ -f "$REPO_DIR/flake.nix" ]; then
        sri="$(flake_dmg_sri || true)"
    fi

    [ -n "$sri" ] || return 0
    sri_to_hex "$sri"
}

verify_dmg_hash() {
    local expected actual
    expected="$(expected_dmg_sha256)"
    [ -n "$expected" ] || error "Set CODEX_DMG_SHA256 or CODEX_DMG_SRI before Apple DMG verification"
    [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || error "Trusted DMG hash is not a hex SHA-256 digest"

    expected="$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')"
    actual="$(shasum -a 256 "$DMG_PATH" | awk '{print $1}')"
    [ "$actual" = "$expected" ] || error "DMG hash mismatch: expected $expected, got $actual"
    info "Verified DMG SHA-256: $actual"
}

mount_dmg() {
    MOUNT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codex-dmg.XXXXXX")"
    hdiutil attach -readonly -nobrowse -mountpoint "$MOUNT_DIR" "$DMG_PATH" >/dev/null
    APP_PATH="$MOUNT_DIR/Codex Installer/Codex.app"
    [ -d "$APP_PATH" ] || error "Expected Codex.app at $APP_PATH"
}

plist_value() {
    /usr/libexec/PlistBuddy -c "Print :$2" "$1"
}

verify_bundle_metadata() {
    local plist="$APP_PATH/Contents/Info.plist"
    local bundle_id sparkle_key

    require_file "$plist"
    bundle_id="$(plist_value "$plist" CFBundleIdentifier)"
    sparkle_key="$(plist_value "$plist" SUPublicEDKey)"

    [ "$bundle_id" = "$EXPECTED_BUNDLE_ID" ] || error "Unexpected bundle id: $bundle_id"
    [ "$sparkle_key" = "$EXPECTED_SPARKLE_KEY" ] || error "Unexpected Sparkle public key: $sparkle_key"

    info "Verified bundle id: $bundle_id"
    info "Verified Sparkle SUPublicEDKey: $sparkle_key"
}

verify_codesign_identity() {
    local details authority team_id
    local details_file

    details_file="$(mktemp "${TMPDIR:-/tmp}/codex-codesign.XXXXXX")"
    codesign -dvvv "$APP_PATH" >"$details_file" 2>&1
    details="$(cat "$details_file")"
    rm -f "$details_file"

    authority="$(printf '%s\n' "$details" | awk -F= '/^Authority=Developer ID Application:/ {print $2; exit}')"
    team_id="$(printf '%s\n' "$details" | awk -F= '/^TeamIdentifier=/ {print $2; exit}')"

    [ "$authority" = "$EXPECTED_DEVELOPER_ID" ] || error "Unexpected Developer ID authority: ${authority:-missing}"
    [ "$team_id" = "$EXPECTED_TEAM_ID" ] || error "Unexpected TeamIdentifier: ${team_id:-missing}"

    codesign --verify --deep --strict --verbose=4 "$APP_PATH"
    info "Verified Developer ID authority: $authority"
    info "Verified Apple TeamIdentifier: $team_id"
}

assess_app_gatekeeper() {
    local output

    output="$(spctl -a -vvv -t exec "$APP_PATH" 2>&1)"
    printf '%s\n' "$output" >&2
    printf '%s\n' "$output" | grep -q 'source=Notarized Developer ID' || error "App exec assessment did not report Notarized Developer ID"

    info "Verified app Gatekeeper assessments"
}

validate_app_staple() {
    xcrun stapler validate "$APP_PATH"
    info "Verified stapled notarization ticket on app bundle"
}

assess_dmg_container() {
    local output status

    hdiutil verify "$DMG_PATH"
    info "Verified DMG container integrity"

    set +e
    output="$(spctl -a -t open --context context:primary-signature -vvv "$DMG_PATH" 2>&1)"
    status=$?
    set -e
    printf '%s\n' "$output" >&2
    if [ "$status" -ne 0 ]; then
        [ "$REQUIRE_DMG_GATEKEEPER" = "1" ] && error "DMG Gatekeeper primary-signature assessment failed"
        warn "DMG Gatekeeper primary-signature assessment failed; contained app checks remain authoritative for Linux repackaging"
    else
        info "Verified DMG Gatekeeper primary-signature assessment"
    fi

    set +e
    output="$(xcrun stapler validate "$DMG_PATH" 2>&1)"
    status=$?
    set -e
    printf '%s\n' "$output" >&2
    if [ "$status" -ne 0 ]; then
        [ "$REQUIRE_DMG_STAPLE" = "1" ] && error "DMG stapler validation failed"
        warn "DMG does not have a required stapled ticket under current settings"
    else
        info "Verified stapled notarization ticket on DMG"
    fi
}

parse_args() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --dmg)
                [ "$#" -ge 2 ] || error "--dmg requires a path"
                DMG_PATH="$2"
                shift 2
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                error "Unknown argument: $1"
                ;;
        esac
    done
}

main() {
    parse_args "$@"

    require_file "$DMG_PATH"
    require_command shasum
    require_command hdiutil
    require_command codesign
    require_command spctl
    require_command xcrun
    require_command /usr/libexec/PlistBuddy

    verify_dmg_hash
    assess_dmg_container
    mount_dmg
    verify_bundle_metadata
    verify_codesign_identity
    assess_app_gatekeeper
    validate_app_staple
    info "Apple DMG verification passed"
}

main "$@"
