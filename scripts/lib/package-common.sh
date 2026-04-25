#!/bin/bash

info() {
    echo "[INFO] $*" >&2
}

error() {
    echo "[ERROR] $*" >&2
    exit 1
}

ensure_file_exists() {
    local path="$1"
    local label="$2"
    [ -f "$path" ] || error "Missing $label: $path"
}

ensure_app_layout() {
    [ -d "$APP_DIR" ] || error "Missing app directory: $APP_DIR. Run ./install.sh first."
    [ -x "$APP_DIR/start.sh" ] || error "Missing launcher: $APP_DIR/start.sh"
}

validate_app_payload_source() {
    local link target
    while IFS= read -r -d '' link; do
        target="$(readlink "$link")" || error "Failed to inspect symlink: $link"
        case "$target" in
            /*|../*|*/../*|*/..|..)
                error "Unsafe symlink in app payload: $link -> $target"
                ;;
        esac
    done < <(find "$APP_DIR" -type l -print0)
}

normalize_app_payload_modes() {
    local app_root="$1"

    find "$app_root" -type d -exec chmod u+rwx,go+rx,go-w,a-s {} +
    find "$app_root" -type f -exec chmod u+rw,go+r,go-w,a-s {} +
}

normalize_package_directory_modes() {
    local root="$1"

    find "$root" -type d -exec chmod u+rwx,go+rx,go-w,a-s {} +
}

resolve_package_version() {
    if [ -n "${PACKAGE_VERSION:-}" ]; then
        printf '%s\n' "$PACKAGE_VERSION"
        return
    fi

    local metadata_file="$APP_DIR/codex-app-version.env"
    [ -f "$metadata_file" ] || error "Missing app version metadata: $metadata_file. Run ./install.sh first or set PACKAGE_VERSION."

    local version=""
    local key value
    while IFS='=' read -r key value || [ -n "$key$value" ]; do
        case "$key" in
            ""|\#*)
                continue
                ;;
            CODEX_APP_PACKAGE_VERSION)
                version="$(printf '%s' "$value" | sed 's/[[:space:]]*$//')"
                break
                ;;
        esac
    done < "$metadata_file"

    [ -n "$version" ] || error "Missing CODEX_APP_PACKAGE_VERSION in $metadata_file"
    if ! [[ "$version" =~ ^[0-9]+(\.[0-9]+){2,3}$ ]]; then
        error "Invalid package version in $metadata_file: $version"
    fi

    printf '%s\n' "$version"
}

validate_identifier() {
    local label="$1"
    local value="$2"
    local pattern="$3"

    [ -n "$value" ] || error "$label must not be empty"
    if ! [[ "$value" =~ $pattern ]]; then
        error "$label contains unsafe characters: $value"
    fi
}

validate_packaging_identifiers() {
    local app_install_name="${APP_INSTALL_NAME:-$PACKAGE_NAME}"
    local launcher_name="${APP_LAUNCHER_NAME:-$app_install_name}"

    validate_identifier "PACKAGE_NAME" "$PACKAGE_NAME" '^[a-z0-9][a-z0-9+._-]*$'
    validate_identifier "APP_INSTALL_NAME" "$app_install_name" '^[A-Za-z0-9][A-Za-z0-9._-]*$'
    validate_identifier "APP_LAUNCHER_NAME" "$launcher_name" '^[A-Za-z0-9][A-Za-z0-9._-]*$'

    if [ -n "${PACKAGE_PROVIDES:-}" ]; then
        validate_identifier "PACKAGE_PROVIDES" "$PACKAGE_PROVIDES" '^[a-z0-9][a-z0-9+._-]*$'
    fi
    if [ -n "${PACKAGE_CONFLICTS:-}" ]; then
        validate_identifier "PACKAGE_CONFLICTS" "$PACKAGE_CONFLICTS" '^[a-z0-9][a-z0-9+._-]*$'
    fi
}

updater_binary_is_stale() {
    local binary="$1"

    [ -x "$binary" ] || return 0

    local source
    for source in "$REPO_DIR/Cargo.toml" "$REPO_DIR/Cargo.lock"; do
        if [ -f "$source" ] && [ "$source" -nt "$binary" ]; then
            return 0
        fi
    done

    while IFS= read -r -d '' source; do
        if [ "$source" -nt "$binary" ]; then
            return 0
        fi
    done < <(find "$REPO_DIR/updater" -type f -print0 2>/dev/null)

    return 1
}

ensure_updater_binary() {
    if [ -x "$UPDATER_BINARY_SOURCE" ] && ! updater_binary_is_stale "$UPDATER_BINARY_SOURCE"; then
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

stage_common_package_files() {
    local root="$1"
    local app_install_name="${APP_INSTALL_NAME:-$PACKAGE_NAME}"
    local app_root="$root/opt/$app_install_name"

    validate_packaging_identifiers

    mkdir -p \
        "$root/opt" \
        "$root/usr/bin" \
        "$root/usr/lib/$app_install_name" \
        "$root/usr/lib/systemd/user" \
        "$root/usr/share/applications" \
        "$root/usr/share/icons/hicolor/256x256/apps"

    validate_app_payload_source
    rm -rf "$app_root"
    cp -aT "$APP_DIR" "$app_root"
    normalize_app_payload_modes "$app_root"
    cp "$DESKTOP_TEMPLATE" "$root/usr/share/applications/$app_install_name.desktop"
    cp "$ICON_SOURCE" "$root/usr/share/icons/hicolor/256x256/apps/$app_install_name.png"
    cp "$UPDATER_BINARY_SOURCE" "$root/usr/bin/codex-app-updater"
    chmod 0755 "$root/usr/bin/codex-app-updater"
    cp "$UPDATER_SERVICE_SOURCE" "$root/usr/lib/systemd/user/codex-app-updater.service"
    chmod 0644 "$root/usr/lib/systemd/user/codex-app-updater.service"
    cp "$PACKAGED_RUNTIME_SOURCE" "$root/usr/lib/$app_install_name/packaged-runtime.sh"
    chmod 0644 "$root/usr/lib/$app_install_name/packaged-runtime.sh"
    normalize_package_directory_modes "$root"
}

stage_update_builder_bundle() {
    local root="$1"
    local app_install_name="${APP_INSTALL_NAME:-$PACKAGE_NAME}"
    local update_builder_root="$root/usr/lib/$app_install_name/update-builder"

    mkdir -p \
        "$update_builder_root/scripts" \
        "$update_builder_root/scripts/lib" \
        "$update_builder_root/packaging/linux" \
        "$update_builder_root/assets"

    cp "$REPO_DIR/install.sh" "$update_builder_root/install.sh"
    cp "$REPO_DIR/scripts/build-deb.sh" "$update_builder_root/scripts/build-deb.sh"
    cp "$REPO_DIR/scripts/build-rpm.sh" "$update_builder_root/scripts/build-rpm.sh"
    cp "$REPO_DIR/scripts/build-pacman.sh" "$update_builder_root/scripts/build-pacman.sh"
    cp "$REPO_DIR/scripts/patch-linux-window-ui.js" "$update_builder_root/scripts/patch-linux-window-ui.js"
    cp "$REPO_DIR/scripts/lib/package-common.sh" "$update_builder_root/scripts/lib/package-common.sh"
    cp "$REPO_DIR/packaging/linux/control" "$update_builder_root/packaging/linux/control"
    cp "$REPO_DIR/packaging/linux/codex-app.spec" "$update_builder_root/packaging/linux/codex-app.spec"
    cp "$REPO_DIR/packaging/linux/codex-app.desktop" "$update_builder_root/packaging/linux/codex-app.desktop"
    cp "$REPO_DIR/packaging/linux/packaged-runtime.sh" "$update_builder_root/packaging/linux/packaged-runtime.sh"
    cp "$REPO_DIR/packaging/linux/codex-app-updater-user-service.sh" \
        "$update_builder_root/packaging/linux/codex-app-updater-user-service.sh"
    cp "$REPO_DIR/packaging/linux/PKGBUILD.template" "$update_builder_root/packaging/linux/PKGBUILD.template"
    cp "$REPO_DIR/packaging/linux/codex-app.install" "$update_builder_root/packaging/linux/codex-app.install"
    cp "$UPDATER_SERVICE_SOURCE" "$update_builder_root/packaging/linux/codex-app-updater.service"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postinst" "$update_builder_root/packaging/linux/codex-app-updater.postinst"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.prerm" "$update_builder_root/packaging/linux/codex-app-updater.prerm"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postrm" "$update_builder_root/packaging/linux/codex-app-updater.postrm"
    cp "$REPO_DIR/assets/codex.png" "$update_builder_root/assets/codex.png"
    normalize_package_directory_modes "$root"
}

write_launcher_stub() {
    local root="$1"
    local app_install_name="${APP_INSTALL_NAME:-$PACKAGE_NAME}"
    local launcher_name="${APP_LAUNCHER_NAME:-$app_install_name}"

    validate_packaging_identifiers

    cat > "$root/usr/bin/$launcher_name" <<SCRIPT
#!/bin/bash
exec /opt/$app_install_name/start.sh "\$@"
SCRIPT
    chmod 0755 "$root/usr/bin/$launcher_name"
}
