#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="${APP_DIR_OVERRIDE:-$REPO_DIR/codex-app}"
DIST_DIR="${DIST_DIR_OVERRIDE:-$REPO_DIR/dist}"
SPEC_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.spec"
DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop"
SERVICE_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.service"
USER_SERVICE_HELPER_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater-user-service.sh"
ICON_SOURCE="$REPO_DIR/assets/codex.png"
PACKAGED_RUNTIME_TEMPLATE="$REPO_DIR/packaging/linux/codex-packaged-runtime.sh"

# Keep the installed update-builder payload aligned with the other package formats.
# shellcheck source=scripts/lib/package-common.sh
. "$REPO_DIR/scripts/lib/package-common.sh"

PACKAGE_NAME="${PACKAGE_NAME:-codex-app}"
PACKAGE_VERSION="${PACKAGE_VERSION:-$(default_package_version)}"
UPDATER_BINARY_SOURCE="${UPDATER_BINARY_SOURCE:-$REPO_DIR/target/release/codex-app-updater}"
UPDATER_SERVICE_SOURCE="${UPDATER_SERVICE_SOURCE:-$SERVICE_TEMPLATE}"
PACKAGED_RUNTIME_SOURCE="${PACKAGED_RUNTIME_SOURCE:-$PACKAGED_RUNTIME_TEMPLATE}"

map_arch() {
    case "$(uname -m)" in
        x86_64)  echo "x86_64" ;;
        aarch64) echo "aarch64" ;;
        armv7l)  echo "armv7hl" ;;
        *)       error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# RPM version must not contain '+'; split PACKAGE_VERSION on '+' into version and release
rpm_version_parts() {
    local base
    base="${PACKAGE_VERSION%%+*}"
    local hash
    hash="${PACKAGE_VERSION#*+}"
    if [ "$base" = "$PACKAGE_VERSION" ]; then
        hash="1"
    fi
    RPM_VERSION="$base"
    RPM_RELEASE="$hash"
}

main() {
    [ -d "$APP_DIR" ] || error "Missing app directory: $APP_DIR. Run ./install.sh first."
    [ -x "$APP_DIR/start.sh" ] || error "Missing launcher: $APP_DIR/start.sh"
    [ -f "$SPEC_TEMPLATE" ] || error "Missing spec template: $SPEC_TEMPLATE"
    [ -f "$DESKTOP_TEMPLATE" ] || error "Missing desktop template: $DESKTOP_TEMPLATE"
    [ -f "$ICON_SOURCE" ] || error "Missing icon: $ICON_SOURCE"
    [ -f "$PACKAGED_RUNTIME_SOURCE" ] || error "Missing packaged launcher runtime helper: $PACKAGED_RUNTIME_SOURCE"
    if package_updater_enabled; then
        [ -f "$UPDATER_SERVICE_SOURCE" ] || error "Missing updater service template: $UPDATER_SERVICE_SOURCE"
        [ -f "$USER_SERVICE_HELPER_TEMPLATE" ] || error "Missing updater user service helper: $USER_SERVICE_HELPER_TEMPLATE"
    fi
    command -v rpmbuild >/dev/null 2>&1 || error "rpmbuild is required (install rpm-build)"

    if package_updater_enabled; then
        ensure_updater_binary
    fi

    local arch
    arch="$(map_arch)"
    rpm_version_parts
    local rpm_ver="$RPM_VERSION"
    local rpm_rel="$RPM_RELEASE"

    local build_root
    build_root="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$build_root'" EXIT

    local staging_root="$build_root/STAGING"

    stage_common_package_files "$staging_root"
    stage_update_builder_bundle "$staging_root"

    cat > "$staging_root/usr/bin/$PACKAGE_NAME" <<SCRIPT
#!/bin/bash
exec /opt/$PACKAGE_NAME/start.sh "\$@"
SCRIPT
    chmod 0755 "$staging_root/usr/bin/$PACKAGE_NAME"

    local spec_file="$build_root/codex-app.spec"
    local updater_requires=""
    local updater_description=""
    local updater_files=""
    local updater_post=""
    local updater_preun=""
    local updater_postun=""
    if package_updater_enabled; then
        updater_requires="Requires:       /usr/bin/7z, polkit, curl, unzip, gcc-c++, make"
        updater_description="Local auto-updates rebuild a Linux package from the upstream Codex.dmg and therefore
use the bundled managed Node.js runtime plus the local packaging toolchain listed in Requires."
        updater_files="/usr/bin/codex-app-updater
/usr/lib/systemd/user/codex-app-updater.service
/usr/share/polkit-1/actions/com.github.nisavid.codex-app.update.policy"
        updater_post="SERVICE_HELPER=/usr/lib/$PACKAGE_NAME/update-builder/packaging/linux/codex-app-updater-user-service.sh
if [ -f \"\$SERVICE_HELPER\" ]; then
    . \"\$SERVICE_HELPER\"
    codex_ensure_user_service_running || true
fi"
        updater_preun="SERVICE_HELPER=/usr/lib/$PACKAGE_NAME/update-builder/packaging/linux/codex-app-updater-user-service.sh
[ -f \"\$SERVICE_HELPER\" ] && . \"\$SERVICE_HELPER\"
if [ \$1 -eq 0 ] && [ -f \"\$SERVICE_HELPER\" ]; then
    codex_cleanup_user_service stop || true
    codex_cleanup_user_service disable || true
fi"
        updater_postun="SERVICE_HELPER=/usr/lib/$PACKAGE_NAME/update-builder/packaging/linux/codex-app-updater-user-service.sh
if [ -f \"\$SERVICE_HELPER\" ]; then
    . \"\$SERVICE_HELPER\"
    codex_reload_user_managers || true
fi"
    fi
    AWK_PACKAGE_NAME="$PACKAGE_NAME" \
    AWK_RPM_VERSION="$rpm_ver" \
    AWK_RPM_RELEASE="$rpm_rel" \
    AWK_RPM_STAGING_DIR="$staging_root" \
    AWK_ARCH="$arch" \
    AWK_UPDATER_REQUIRES="$updater_requires" \
    AWK_UPDATER_DESCRIPTION="$updater_description" \
    AWK_UPDATER_FILES="$updater_files" \
    AWK_UPDATER_POST="$updater_post" \
    AWK_UPDATER_PREUN="$updater_preun" \
    AWK_UPDATER_POSTUN="$updater_postun" \
    awk '
        function emit_env(name) {
            if (ENVIRON[name] != "") {
                print ENVIRON[name]
            }
        }
        {
            if ($0 == "__UPDATER_REQUIRES__") { emit_env("AWK_UPDATER_REQUIRES"); next }
            if ($0 == "__UPDATER_DESCRIPTION__") { emit_env("AWK_UPDATER_DESCRIPTION"); next }
            if ($0 == "__UPDATER_FILES__") { emit_env("AWK_UPDATER_FILES"); next }
            if ($0 == "__UPDATER_POST__") { emit_env("AWK_UPDATER_POST"); next }
            if ($0 == "__UPDATER_PREUN__") { emit_env("AWK_UPDATER_PREUN"); next }
            if ($0 == "__UPDATER_POSTUN__") { emit_env("AWK_UPDATER_POSTUN"); next }
            gsub(/__PACKAGE_NAME__/, ENVIRON["AWK_PACKAGE_NAME"])
            gsub(/__RPM_VERSION__/, ENVIRON["AWK_RPM_VERSION"])
            gsub(/__RPM_RELEASE__/, ENVIRON["AWK_RPM_RELEASE"])
            gsub(/__RPM_STAGING_DIR__/, ENVIRON["AWK_RPM_STAGING_DIR"])
            gsub(/__ARCH__/, ENVIRON["AWK_ARCH"])
            print
        }
    ' "$SPEC_TEMPLATE" > "$spec_file"

    local rpmbuild_dir="$build_root/rpmbuild"
    mkdir -p \
        "$rpmbuild_dir/RPMS" \
        "$rpmbuild_dir/SRPMS" \
        "$rpmbuild_dir/BUILD" \
        "$rpmbuild_dir/SOURCES" \
        "$rpmbuild_dir/SPECS"

    mkdir -p "$DIST_DIR"
    info "Building $PACKAGE_NAME-${rpm_ver}-${rpm_rel}.${arch}.rpm"
    rpmbuild -bb \
        --define "_rpmdir $rpmbuild_dir/RPMS" \
        --define "_srcrpmdir $rpmbuild_dir/SRPMS" \
        --define "_builddir $rpmbuild_dir/BUILD" \
        --define "_sourcedir $rpmbuild_dir/SOURCES" \
        --define "_specdir $build_root" \
        --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
        "$spec_file" >&2

    local rpm_file
    rpm_file="$(find "$rpmbuild_dir/RPMS" -name "*.rpm" | head -n 1)"
    [ -f "$rpm_file" ] || error "rpmbuild did not produce an RPM"

    local output_file="$DIST_DIR/${PACKAGE_NAME}-${rpm_ver}-${rpm_rel}.${arch}.rpm"
    cp "$rpm_file" "$output_file"
    info "Built package: $output_file"
}

main "$@"
