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

default_package_version() {
    local version_file="$APP_DIR/codex-app-version.env"
    local version=""

    if [ ! -f "$version_file" ]; then
        error "Missing $version_file. Run ./install.sh first so package versions align with the DMG app version."
    fi

    version="$(sed -n 's/^CODEX_APP_PACKAGE_VERSION=//p' "$version_file" | head -n 1)"
    version="${version#\'}"
    version="${version%\'}"
    version="${version#\"}"
    version="${version%\"}"
    if [[ "$version" =~ ^[0-9]+(\.[0-9]+){2,3}$ ]]; then
        echo "$version"
        return
    fi

    error "Invalid CODEX_APP_PACKAGE_VERSION in $version_file: $version"
}

sed_escape_replacement() {
    printf '%s' "$1" | sed -e 's/[\\\/&]/\\&/g'
}

validate_no_newline() {
    local name="$1"
    local value="$2"

    case "$value" in
    *$'\n'*|*$'\r'*)
        error "$name must not contain newlines"
        ;;
    esac
}

validate_package_inputs() {
    [[ "$PACKAGE_NAME" =~ ^[a-z0-9][a-z0-9+._-]*$ ]] || \
        error "PACKAGE_NAME must match ^[a-z0-9][a-z0-9+._-]*$: $PACKAGE_NAME"
    validate_no_newline "PACKAGE_DISPLAY_NAME" "${PACKAGE_DISPLAY_NAME:-Codex App}"
    validate_no_newline "PACKAGE_COMMENT" "${PACKAGE_COMMENT:-Run Codex App on Linux}"
}

render_desktop_entry() {
    local target="$1"
    local package_name
    local display_name
    local comment

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    display_name="$(sed_escape_replacement "${PACKAGE_DISPLAY_NAME:-Codex App}")"
    comment="$(sed_escape_replacement "${PACKAGE_COMMENT:-Run Codex App on Linux}")"

    sed \
        -e "s/codex-app-updater/__CODEX_APP_UPDATER__/g" \
        -e "s/codex-app/$package_name/g" \
        -e "s/__CODEX_APP_UPDATER__/codex-app-updater/g" \
        -e "s/^Name=.*/Name=$display_name/g" \
        -e "s/^Comment=.*/Comment=$comment/g" \
        "$DESKTOP_TEMPLATE" > "$target"
    chmod 0644 "$target"
}

render_packaged_runtime_helper() {
    local target="$1"
    local package_name

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    sed \
        -e "s/CHROME_DESKTOP=\"codex-app.desktop\"/CHROME_DESKTOP=\"$package_name.desktop\"/" \
        -e "s|BAMF_DESKTOP_FILE_HINT=\"/usr/share/applications/codex-app.desktop\"|BAMF_DESKTOP_FILE_HINT=\"/usr/share/applications/$package_name.desktop\"|" \
        "$PACKAGED_RUNTIME_SOURCE" > "$target"
    chmod 0644 "$target"
}

validate_app_payload_source() {
    local app_root
    local link
    local link_dir
    local resolved_target
    local target

    app_root="$(realpath -m "$APP_DIR")"
    while IFS= read -r -d '' link; do
        target="$(readlink "$link")" || error "Failed to read symlink: $link"
        link_dir="$(dirname "$link")"
        case "$target" in
        /*) error "Absolute symlinks are not allowed in app payload: $link -> $target" ;;
        *) resolved_target="$(realpath -m "$link_dir/$target")" ;;
        esac

        [ -e "$resolved_target" ] || error "Broken symlink in app payload: $link -> $target"

        case "$resolved_target" in
        "$app_root"|"$app_root"/*)
            ;;
        *)
            error "Unsafe symlink in app payload: $link -> $target"
            ;;
        esac
    done < <(find "$APP_DIR" -type l -print0)
}

normalize_app_payload_modes() {
    local app_root="$1"

    find "$app_root" -exec chmod u-s,g-s,o-t {} +
    chmod -R u+rwX,go+rX,go-w "$app_root"
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
    local app_root="$root/opt/$PACKAGE_NAME"
    local support_root="$root/usr/lib/$PACKAGE_NAME"
    local polkit_policy="$REPO_DIR/packaging/linux/com.github.nisavid.codex-app.update.policy"

    validate_package_inputs
    validate_app_payload_source
    ensure_file_exists "$polkit_policy" "polkit policy"

    mkdir -p \
        "$root/opt" \
        "$root/usr/bin" \
        "$support_root" \
        "$root/usr/lib/systemd/user" \
        "$root/usr/share/applications" \
        "$root/usr/share/icons/hicolor/256x256/apps" \
        "$root/usr/share/polkit-1/actions"

    rm -rf "$app_root"
    cp -aT "$APP_DIR" "$app_root"
    normalize_app_payload_modes "$app_root"
    mkdir -p "$app_root/.codex-linux"
    cp "$ICON_SOURCE" "$app_root/.codex-linux/$PACKAGE_NAME.png"
    render_desktop_entry "$root/usr/share/applications/$PACKAGE_NAME.desktop"
    cp "$ICON_SOURCE" "$root/usr/share/icons/hicolor/256x256/apps/$PACKAGE_NAME.png"
    cp "$UPDATER_BINARY_SOURCE" "$root/usr/bin/codex-app-updater"
    chmod 0755 "$root/usr/bin/codex-app-updater"
    cp "$UPDATER_SERVICE_SOURCE" "$root/usr/lib/systemd/user/codex-app-updater.service"
    chmod 0644 "$root/usr/lib/systemd/user/codex-app-updater.service"
    cp "$polkit_policy" "$root/usr/share/polkit-1/actions/com.github.nisavid.codex-app.update.policy"
    chmod 0644 "$root/usr/share/polkit-1/actions/com.github.nisavid.codex-app.update.policy"
    render_packaged_runtime_helper "$support_root/packaged-runtime.sh"
}

stage_update_builder_bundle() {
    local root="$1"
    local update_builder_root="$root/usr/lib/$PACKAGE_NAME/update-builder"
    local node_runtime_source="$APP_DIR/resources/node-runtime"

    mkdir -p \
        "$update_builder_root/scripts" \
        "$update_builder_root/launcher" \
        "$update_builder_root/packaging/linux" \
        "$update_builder_root/assets"

    cp "$REPO_DIR/install.sh" "$update_builder_root/install.sh"
    cp "$REPO_DIR/launcher/start.sh.template" "$update_builder_root/launcher/start.sh.template"
    cp "$REPO_DIR/Cargo.toml" "$update_builder_root/Cargo.toml"
    cp "$REPO_DIR/Cargo.lock" "$update_builder_root/Cargo.lock"
    cp -r "$REPO_DIR/computer-use-linux" "$update_builder_root/computer-use-linux"
    cp -r "$REPO_DIR/updater" "$update_builder_root/updater"
    mkdir -p "$update_builder_root/plugins/openai-bundled/plugins"
    cp -r "$REPO_DIR/plugins/openai-bundled/plugins/computer-use" \
        "$update_builder_root/plugins/openai-bundled/plugins/computer-use"
    cp "$REPO_DIR/scripts/build-deb.sh" "$update_builder_root/scripts/build-deb.sh"
    cp "$REPO_DIR/scripts/build-rpm.sh" "$update_builder_root/scripts/build-rpm.sh"
    cp "$REPO_DIR/scripts/build-pacman.sh" "$update_builder_root/scripts/build-pacman.sh"
    cp "$REPO_DIR/scripts/rebuild-candidate.sh" "$update_builder_root/scripts/rebuild-candidate.sh"
    cp "$REPO_DIR/scripts/patch-linux-window-ui.js" "$update_builder_root/scripts/patch-linux-window-ui.js"
    cp -r "$REPO_DIR/scripts/lib" "$update_builder_root/scripts/lib"
    cp "$REPO_DIR/packaging/linux/control" "$update_builder_root/packaging/linux/control"
    cp "$REPO_DIR/packaging/linux/codex-app.spec" "$update_builder_root/packaging/linux/codex-app.spec"
    cp "$REPO_DIR/packaging/linux/codex-app.desktop" "$update_builder_root/packaging/linux/codex-app.desktop"
    cp "$REPO_DIR/packaging/linux/codex-packaged-runtime.sh" "$update_builder_root/packaging/linux/codex-packaged-runtime.sh"
    cp "$REPO_DIR/packaging/linux/com.github.nisavid.codex-app.update.policy" \
        "$update_builder_root/packaging/linux/com.github.nisavid.codex-app.update.policy"
    cp "$REPO_DIR/packaging/linux/codex-app-updater-user-service.sh" \
        "$update_builder_root/packaging/linux/codex-app-updater-user-service.sh"
    cp "$REPO_DIR/packaging/linux/PKGBUILD.template" "$update_builder_root/packaging/linux/PKGBUILD.template"
    cp "$REPO_DIR/packaging/linux/codex-app.install" "$update_builder_root/packaging/linux/codex-app.install"
    cp "$UPDATER_SERVICE_SOURCE" "$update_builder_root/packaging/linux/codex-app-updater.service"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postinst" "$update_builder_root/packaging/linux/codex-app-updater.postinst"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.prerm" "$update_builder_root/packaging/linux/codex-app-updater.prerm"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postrm" "$update_builder_root/packaging/linux/codex-app-updater.postrm"
    cp "$REPO_DIR/assets/codex.png" "$update_builder_root/assets/codex.png"
    if [ -d "$node_runtime_source" ]; then
        cp -a "$node_runtime_source" "$update_builder_root/node-runtime"
    else
        error "Missing managed Node.js runtime: $node_runtime_source. Run ./install.sh first."
    fi
}

write_launcher_stub() {
    local root="$1"

    cat > "$root/usr/bin/$PACKAGE_NAME" <<SCRIPT
#!/bin/bash
exec /opt/$PACKAGE_NAME/start.sh "\$@"
SCRIPT
    chmod 0755 "$root/usr/bin/$PACKAGE_NAME"
}
