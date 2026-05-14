#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$REPO_DIR/scripts/lib/package-common.sh"
APP_DIR="${APP_DIR_OVERRIDE:-$REPO_DIR/codex-app}"
PKG_ROOT="${PKG_ROOT_OVERRIDE:-$REPO_DIR/dist/deb-root}"
DIST_DIR="${DIST_DIR_OVERRIDE:-$REPO_DIR/dist}"
CONTROL_TEMPLATE="$REPO_DIR/packaging/linux/control"
DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop"
SERVICE_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.service"
USER_SERVICE_HELPER_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater-user-service.sh"
ICON_SOURCE="$REPO_DIR/assets/codex.png"
PRERM_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.prerm"
POSTRM_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.postrm"
POSTINST_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.postinst"
PACKAGED_RUNTIME_TEMPLATE="$REPO_DIR/packaging/linux/codex-packaged-runtime.sh"

PACKAGE_NAME="${PACKAGE_NAME:-codex-app}"
PACKAGE_VERSION="${PACKAGE_VERSION:-$(default_package_version)}"
UPDATER_BINARY_SOURCE="${UPDATER_BINARY_SOURCE:-$REPO_DIR/target/release/codex-app-updater}"
UPDATER_SERVICE_SOURCE="${UPDATER_SERVICE_SOURCE:-$SERVICE_TEMPLATE}"
PACKAGED_RUNTIME_SOURCE="${PACKAGED_RUNTIME_SOURCE:-$PACKAGED_RUNTIME_TEMPLATE}"

map_arch() {
    case "$(dpkg --print-architecture)" in
        amd64|arm64|armhf)
            dpkg --print-architecture
            ;;
        *)
            error "Unsupported Debian architecture: $(dpkg --print-architecture)"
            ;;
    esac
}

main() {
    ensure_app_layout
    ensure_file_exists "$CONTROL_TEMPLATE" "control template"
    ensure_file_exists "$DESKTOP_TEMPLATE" "desktop template"
    ensure_file_exists "$ICON_SOURCE" "icon"
    ensure_file_exists "$PACKAGED_RUNTIME_SOURCE" "packaged launcher runtime helper"
    if package_updater_enabled; then
        ensure_file_exists "$UPDATER_SERVICE_SOURCE" "updater service template"
        ensure_file_exists "$USER_SERVICE_HELPER_TEMPLATE" "updater user service helper"
        ensure_file_exists "$PRERM_TEMPLATE" "Debian prerm template"
        ensure_file_exists "$POSTRM_TEMPLATE" "Debian postrm template"
        ensure_file_exists "$POSTINST_TEMPLATE" "Debian postinst template"
    fi
    command -v dpkg-deb >/dev/null 2>&1 || error "dpkg-deb is required"
    command -v dpkg >/dev/null 2>&1 || error "dpkg is required"

    if package_updater_enabled; then
        ensure_updater_binary
    fi

    local arch output_file
    arch="$(map_arch)"
    output_file="$DIST_DIR/${PACKAGE_NAME}_${PACKAGE_VERSION}_${arch}.deb"

    info "Preparing package root at $PKG_ROOT"
    rm -rf "$PKG_ROOT"
    mkdir -p \
        "$PKG_ROOT/DEBIAN" \
        "$PKG_ROOT/opt"

    stage_common_package_files "$PKG_ROOT"
    stage_update_builder_bundle "$PKG_ROOT"
    write_launcher_stub "$PKG_ROOT"

    local deb_depends
    local updater_description=""
    deb_depends="python3, libasound2 | libasound2t64, libatk-bridge2.0-0, libatk1.0-0, libc6, libcairo2, libcups2t64 | libcups2, libdbus-1-3, libdrm2, libgbm1, libglib2.0-0t64 | libglib2.0-0, libgtk-3-0t64 | libgtk-3-0, libnspr4, libnss3, libpango-1.0-0, libstdc++6, libx11-6, libx11-xcb1, libxcb-dri3-0, libxcb1, libxcomposite1, libxdamage1, libxext6, libxfixes3, libxkbcommon0, libxrandr2"
    if package_updater_enabled; then
        deb_depends="build-essential, curl, dpkg, p7zip-full, pkexec | policykit-1, polkitd | policykit-1, $deb_depends, unzip"
        updater_description=" Local auto-updates rebuild a Linux package from the upstream Codex.dmg and therefore
 use the bundled managed Node.js runtime plus the local packaging toolchain listed in Depends."
    fi
    AWK_PACKAGE_NAME="$PACKAGE_NAME" \
    AWK_VERSION="$PACKAGE_VERSION" \
    AWK_ARCH="$arch" \
    AWK_DEB_DEPENDS="$deb_depends" \
    AWK_UPDATER_DESCRIPTION="$updater_description" \
    awk '
        function emit_env(name) {
            if (ENVIRON[name] != "") {
                print ENVIRON[name]
            }
        }
        {
            if ($0 == "__UPDATER_DESCRIPTION__") { emit_env("AWK_UPDATER_DESCRIPTION"); next }
            gsub(/__PACKAGE_NAME__/, ENVIRON["AWK_PACKAGE_NAME"])
            gsub(/__VERSION__/, ENVIRON["AWK_VERSION"])
            gsub(/__ARCH__/, ENVIRON["AWK_ARCH"])
            gsub(/__DEB_DEPENDS__/, ENVIRON["AWK_DEB_DEPENDS"])
            print
        }
    ' "$CONTROL_TEMPLATE" > "$PKG_ROOT/DEBIAN/control"
    chmod 0644 "$PKG_ROOT/DEBIAN/control"
    if package_updater_enabled; then
        sed \
            -e "s|/opt/codex-app|/opt/$PACKAGE_NAME|g" \
            -e "s|/usr/lib/codex-app|/usr/lib/$PACKAGE_NAME|g" \
            "$POSTINST_TEMPLATE" > "$PKG_ROOT/DEBIAN/postinst"
        cp "$PRERM_TEMPLATE" "$PKG_ROOT/DEBIAN/prerm"
        cp "$POSTRM_TEMPLATE" "$PKG_ROOT/DEBIAN/postrm"
        chmod 0755 "$PKG_ROOT/DEBIAN/postinst" "$PKG_ROOT/DEBIAN/prerm" "$PKG_ROOT/DEBIAN/postrm"
    fi

    mkdir -p "$DIST_DIR"
    info "Building $output_file"
    dpkg-deb --root-owner-group --build "$PKG_ROOT" "$output_file" >&2
    info "Inspecting package metadata"
    dpkg-deb -I "$output_file" >&2
    info "Inspecting package contents"
    dpkg-deb -c "$output_file" >&2
    info "Built package: $output_file"
}

main "$@"
