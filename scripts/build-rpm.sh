#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$REPO_DIR/scripts/lib/package-common.sh"
APP_DIR="${APP_DIR_OVERRIDE:-$REPO_DIR/codex-app}"
DIST_DIR="${DIST_DIR_OVERRIDE:-$REPO_DIR/dist}"
SPEC_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.spec"
DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop"
SERVICE_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.service"
USER_SERVICE_HELPER_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater-user-service.sh"
ICON_SOURCE="$REPO_DIR/assets/codex.png"
PACKAGED_RUNTIME_TEMPLATE="$REPO_DIR/packaging/linux/packaged-runtime.sh"

PACKAGE_NAME="${PACKAGE_NAME:-codex-app}"
PACKAGE_VERSION="$(resolve_package_version)"
UPDATER_BINARY_SOURCE="${UPDATER_BINARY_SOURCE:-$REPO_DIR/target/release/codex-app-updater}"
UPDATER_SERVICE_SOURCE="${UPDATER_SERVICE_SOURCE:-$SERVICE_TEMPLATE}"
PACKAGED_RUNTIME_SOURCE="${PACKAGED_RUNTIME_SOURCE:-$PACKAGED_RUNTIME_TEMPLATE}"
UPDATE_BUILDER_ROOT_PLACEHOLDER="__UPDATE_BUILDER_ROOT__"

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

ensure_updater_binary() {
    if [ -x "$UPDATER_BINARY_SOURCE" ]; then
        return
    fi

    [ -f "$REPO_DIR/Cargo.toml" ] || error "Missing updater binary: $UPDATER_BINARY_SOURCE"
    command -v cargo >/dev/null 2>&1 || error "cargo is required to build codex-app-updater.
Install the Rust toolchain:
  bash scripts/install-deps.sh        # auto-installs via rustup
  # or manually: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"

    info "Building codex-app-updater release binary"
    cargo build --release -p codex-app-updater >&2
    [ -x "$UPDATER_BINARY_SOURCE" ] || error "Failed to build updater binary: $UPDATER_BINARY_SOURCE"
}

main() {
    [ -d "$APP_DIR" ] || error "Missing app directory: $APP_DIR. Run ./install.sh first."
    [ -x "$APP_DIR/start.sh" ] || error "Missing launcher: $APP_DIR/start.sh"
    [ -f "$SPEC_TEMPLATE" ] || error "Missing spec template: $SPEC_TEMPLATE"
    [ -f "$DESKTOP_TEMPLATE" ] || error "Missing desktop template: $DESKTOP_TEMPLATE"
    [ -f "$UPDATER_SERVICE_SOURCE" ] || error "Missing updater service template: $UPDATER_SERVICE_SOURCE"
    [ -f "$USER_SERVICE_HELPER_TEMPLATE" ] || error "Missing updater user service helper: $USER_SERVICE_HELPER_TEMPLATE"
    [ -f "$ICON_SOURCE" ] || error "Missing icon: $ICON_SOURCE"
    [ -f "$PACKAGED_RUNTIME_SOURCE" ] || error "Missing packaged launcher runtime helper: $PACKAGED_RUNTIME_SOURCE"
    command -v rpmbuild >/dev/null 2>&1 || error "rpmbuild is required (install rpm-build)"

    ensure_updater_binary

    local arch
    arch="$(map_arch)"
    rpm_version_parts
    local rpm_ver="$RPM_VERSION"
    local rpm_rel="$RPM_RELEASE"

    local build_root
    build_root="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$build_root'" EXIT

    local staging_root="${TEST_RPM_STAGING:-$build_root/STAGING}"
    local update_builder_dir="$staging_root/usr/lib/$PACKAGE_NAME/update-builder"

    mkdir -p \
        "$staging_root/opt/$PACKAGE_NAME" \
        "$staging_root/usr/bin" \
        "$staging_root/usr/lib/$PACKAGE_NAME" \
        "$staging_root/usr/lib/systemd/user" \
        "$staging_root/usr/share/applications" \
        "$staging_root/usr/share/icons/hicolor/256x256/apps"

    cp -a "$APP_DIR/." "$staging_root/opt/$PACKAGE_NAME/"
    cp "$DESKTOP_TEMPLATE" "$staging_root/usr/share/applications/$PACKAGE_NAME.desktop"
    cp "$ICON_SOURCE" "$staging_root/usr/share/icons/hicolor/256x256/apps/$PACKAGE_NAME.png"
    cp "$UPDATER_BINARY_SOURCE" "$staging_root/usr/bin/codex-app-updater"
    chmod 0755 "$staging_root/usr/bin/codex-app-updater"
    cp "$UPDATER_SERVICE_SOURCE" "$staging_root/usr/lib/systemd/user/codex-app-updater.service"
    chmod 0644 "$staging_root/usr/lib/systemd/user/codex-app-updater.service"
    cp "$PACKAGED_RUNTIME_SOURCE" "$staging_root/usr/lib/$PACKAGE_NAME/packaged-runtime.sh"
    chmod 0644 "$staging_root/usr/lib/$PACKAGE_NAME/packaged-runtime.sh"

    mkdir -p \
        "$update_builder_dir/scripts" \
        "$update_builder_dir/scripts/lib" \
        "$update_builder_dir/packaging/linux" \
        "$update_builder_dir/assets"
    cp "$REPO_DIR/install.sh" "$update_builder_dir/install.sh"
    cp "$REPO_DIR/scripts/build-rpm.sh" "$update_builder_dir/scripts/build-rpm.sh"
    cp "$REPO_DIR/scripts/build-deb.sh" "$update_builder_dir/scripts/build-deb.sh"
    cp "$REPO_DIR/scripts/build-pacman.sh" "$update_builder_dir/scripts/build-pacman.sh"
    cp "$REPO_DIR/scripts/patch-linux-window-ui.js" "$update_builder_dir/scripts/patch-linux-window-ui.js"
    cp "$REPO_DIR/scripts/lib/package-common.sh" "$update_builder_dir/scripts/lib/package-common.sh"
    cp "$REPO_DIR/packaging/linux/codex-app.spec" "$update_builder_dir/packaging/linux/codex-app.spec"
    cp "$REPO_DIR/packaging/linux/control" "$update_builder_dir/packaging/linux/control"
    cp "$REPO_DIR/packaging/linux/codex-app.desktop" "$update_builder_dir/packaging/linux/codex-app.desktop"
    cp "$PACKAGED_RUNTIME_SOURCE" "$update_builder_dir/packaging/linux/packaged-runtime.sh"
    cp "$USER_SERVICE_HELPER_TEMPLATE" \
        "$update_builder_dir/packaging/linux/codex-app-updater-user-service.sh"
    cp "$REPO_DIR/packaging/linux/PKGBUILD.template" "$update_builder_dir/packaging/linux/PKGBUILD.template"
    cp "$REPO_DIR/packaging/linux/codex-app.install" "$update_builder_dir/packaging/linux/codex-app.install"
    cp "$UPDATER_SERVICE_SOURCE" "$update_builder_dir/packaging/linux/codex-app-updater.service"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postinst" "$update_builder_dir/packaging/linux/codex-app-updater.postinst"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.prerm" "$update_builder_dir/packaging/linux/codex-app-updater.prerm"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postrm" "$update_builder_dir/packaging/linux/codex-app-updater.postrm"
    cp "$REPO_DIR/assets/codex.png" "$update_builder_dir/assets/codex.png"

    cat > "$staging_root/usr/bin/$PACKAGE_NAME" <<SCRIPT
#!/bin/bash
exec /opt/$PACKAGE_NAME/start.sh "\$@"
SCRIPT
    chmod 0755 "$staging_root/usr/bin/$PACKAGE_NAME"

    local spec_file="$build_root/codex-app.spec"
    sed \
        -e "s/__PACKAGE_NAME__/$PACKAGE_NAME/g" \
        -e "s/__RPM_VERSION__/$rpm_ver/g" \
        -e "s/__RPM_RELEASE__/$rpm_rel/g" \
        -e "s|__RPM_STAGING_DIR__|$staging_root|g" \
        -e "s/__ARCH__/$arch/g" \
        "$SPEC_TEMPLATE" > "$spec_file"

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
