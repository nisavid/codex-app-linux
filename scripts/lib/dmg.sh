#!/bin/bash
# Codex.dmg download, extraction, and Electron-version detection from app metadata.
#
# Sourced by install.sh. Do not run directly.
# shellcheck shell=bash

# ---- Download or find Codex DMG ----
get_dmg() {
    local dmg_dest="$CACHED_DMG_PATH"

    # Reuse existing DMG
    if [ -s "$dmg_dest" ]; then
        info "Using cached DMG: $dmg_dest ($(du -h "$dmg_dest" | cut -f1))"
        echo "$dmg_dest"
        return
    fi

    info "Downloading Codex App DMG..."
    local dmg_url="https://persistent.oaistatic.com/codex-app-prod/Codex.dmg"
    info "URL: $dmg_url"

    if ! curl --fail -L --progress-bar --max-time 600 --connect-timeout 30 \
            -o "$dmg_dest" "$dmg_url"; then
        rm -f "$dmg_dest"
        error "Download failed. Download manually and place as: $dmg_dest"
    fi

    if [ ! -s "$dmg_dest" ]; then
        rm -f "$dmg_dest"
        error "Download produced empty file. Download manually and place as: $dmg_dest"
    fi

    info "Saved: $dmg_dest ($(du -h "$dmg_dest" | cut -f1))"
    echo "$dmg_dest"
}

# ---- Extract app from DMG ----
extract_dmg() {
    local dmg_path="$1"
    info "Extracting DMG with 7z..."

    local extract_dir="$WORK_DIR/dmg-extract"
    local seven_log="$WORK_DIR/7z.log"
    local seven_zip_status=0

    mkdir -p "$extract_dir"
    if "$SEVEN_ZIP_CMD" x -y -snl "$dmg_path" -o"$extract_dir" >"$seven_log" 2>&1; then
        :
    else
        seven_zip_status=$?
    fi

    local app_dir
    app_dir=$(find "$extract_dir" -maxdepth 3 -name "*.app" -type d -print -quit)

    if [ "$seven_zip_status" -ne 0 ]; then
        if [ -n "$app_dir" ]; then
            warn "7z exited with code $seven_zip_status but app bundle was found; continuing"
            warn "$(tail -n 5 "$seven_log" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g')"
        else
            cat "$seven_log" >&2
            error "Failed to extract DMG"
        fi
    fi

    [ -n "$app_dir" ] || error "Could not find .app bundle in DMG"

    info "Found: $(basename "$app_dir")"
    echo "$app_dir"
}

# ---- Detect Electron version from DMG ----
sanitize_electron_version() {
    local value="$1"
    value="${value#v}"
    value="${value#^}"
    value="${value#~}"

    if [[ "$value" =~ ^[0-9]+(\.[0-9]+){2}([.-][0-9A-Za-z]+)*$ ]]; then
        echo "$value"
        return 0
    fi

    return 1
}

sanitize_app_package_version() {
    local value="$1"

    if [[ "$value" =~ ^[0-9]+(\.[0-9]+){2,3}$ ]]; then
        echo "$value"
        return 0
    fi

    return 1
}

write_app_version_metadata() {
    local app_dir="$1"
    local plist_file="$app_dir/Contents/Info.plist"
    local detected_version=""
    local package_version=""
    local bundle_version=""

    [ -f "$plist_file" ] || error "Missing app Info.plist: $plist_file"

    detected_version=$(python3 - "$plist_file" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "rb") as handle:
    data = plistlib.load(handle)
print(data.get("CFBundleShortVersionString", ""))
print(data.get("CFBundleVersion", ""))
PY
)
    package_version="$(printf '%s\n' "$detected_version" | sed -n '1p')"
    bundle_version="$(printf '%s\n' "$detected_version" | sed -n '2p')"

    if ! package_version="$(sanitize_app_package_version "$package_version")"; then
        error "Invalid Codex App package version in DMG Info.plist: $(printf '%s\n' "$detected_version" | sed -n '1p')"
    fi

    mkdir -p "$INSTALL_DIR"
    cat > "$INSTALL_DIR/codex-app-version.env" <<EOF
CODEX_APP_PACKAGE_VERSION=$(shell_quote "$package_version")
CODEX_APP_BUNDLE_VERSION=$(shell_quote "$bundle_version")
EOF
    info "Detected Codex App package version from DMG: $package_version"
}

detect_electron_version() {
    local app_dir="$1"
    local detected=""
    local detected_version=""
    local plist_file="$app_dir/Contents/Frameworks/Electron Framework.framework/Versions/A/Resources/Info.plist"

    if [ -f "$plist_file" ]; then
        detected=$(python3 - "$plist_file" <<'PY' 2>/dev/null || true
import plistlib
import sys

with open(sys.argv[1], "rb") as handle:
    print(plistlib.load(handle).get("CFBundleVersion", ""))
PY
)
        if detected_version=$(sanitize_electron_version "$detected"); then
            ELECTRON_VERSION="$detected_version"
            info "Detected Electron version from DMG: $ELECTRON_VERSION"
            return 0
        elif [ -n "$detected" ]; then
            warn "Ignoring invalid Electron version from DMG: $detected"
        fi
    fi

    local resources_dir="$app_dir/Contents/Resources"
    if [ -f "$resources_dir/app.asar" ]; then
        detected=$(npx --yes asar extract-file "$resources_dir/app.asar" package.json 2>/dev/null |
            node -e '
const fs = require("node:fs");
const pkg = JSON.parse(fs.readFileSync(0, "utf8"));
process.stdout.write(String(pkg.devDependencies?.electron ?? pkg.dependencies?.electron ?? ""));
' 2>/dev/null || true)
        if detected_version=$(sanitize_electron_version "$detected"); then
            ELECTRON_VERSION="$detected_version"
            info "Detected Electron version from package.json: $ELECTRON_VERSION"
            return 0
        elif [ -n "$detected" ]; then
            warn "Ignoring invalid Electron version from package.json: $detected"
        fi
    fi

    warn "Could not auto-detect Electron version; using fallback $ELECTRON_VERSION"
    return 0
}
