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
        error "Missing $version_file. Run ./install.sh first so package versions align with the official app bundle version."
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
    package_with_updater_value >/dev/null
    validate_no_newline "PACKAGE_DISPLAY_NAME" "${PACKAGE_DISPLAY_NAME:-Codex App}"
    validate_no_newline "PACKAGE_COMMENT" "${PACKAGE_COMMENT:-Run Codex App on Linux}"
}

normalize_package_updater_value() {
    case "$1" in
    1|true|True|TRUE|yes|Yes|YES|on|On|ON)
        echo 1
        ;;
    0|false|False|FALSE|no|No|NO|off|Off|OFF)
        echo 0
        ;;
    *)
        error "PACKAGE_WITH_UPDATER must be 1 or 0"
        ;;
    esac
}

package_with_updater_value() {
    local canonical="${PACKAGE_WITH_UPDATER:-}"
    local legacy="${PACKAGE_ENABLE_UPDATER:-}"

    if [ -n "$canonical" ] && [ -n "$legacy" ]; then
        canonical="$(normalize_package_updater_value "$canonical")"
        legacy="$(normalize_package_updater_value "$legacy")"
        [ "$canonical" = "$legacy" ] || \
            error "PACKAGE_WITH_UPDATER and PACKAGE_ENABLE_UPDATER disagree"
        echo "$canonical"
        return
    fi

    if [ -n "$canonical" ]; then
        normalize_package_updater_value "$canonical"
    elif [ -n "$legacy" ]; then
        normalize_package_updater_value "$legacy"
    else
        echo 1
    fi
}

package_with_updater_enabled() {
    [ "$(package_with_updater_value)" = "1" ]
}

package_node_binary() {
    local managed_node="${APP_DIR:-}/resources/node-runtime/bin/node"
    if [ -x "$managed_node" ] && [ "$("$managed_node" -e 'process.stdout.write("ok")' 2>/dev/null || true)" = "ok" ]; then
        printf '%s\n' "$managed_node"
        return 0
    fi

    command -v node >/dev/null 2>&1 || error "node is required"
    command -v node
}

clear_update_builder_port_integration_config() {
    local update_builder_root="$1"

    rm -f \
        "$update_builder_root/port-integrations/integrations.json" \
        "$update_builder_root/port-integrations/features.json"
}

stage_update_builder_resolved_port_integration_config() {
    local update_builder_root="$1"
    local helper="$REPO_DIR/scripts/lib/port-integrations.js"
    local node_bin
    local config_dir="$update_builder_root/.codex-linux"
    local config_path="$config_dir/port-integrations.json"

    [ -f "$helper" ] || error "Missing port integrations helper: $helper"

    mkdir -p "$config_dir"
    node_bin="$(package_node_binary)"
    "$node_bin" "$helper" --resolved-config-json > "$config_path"
}

port_integrations_root_path() {
    local helper="$REPO_DIR/scripts/lib/port-integrations.js"
    local node_bin

    [ -f "$helper" ] || error "Missing port integrations helper: $helper"

    node_bin="$(package_node_binary)"
    "$node_bin" "$helper" --integrations-root
}

stage_update_builder_port_integrations_tree() {
    local update_builder_root="$1"
    local source_root
    local target="$update_builder_root/port-integrations"

    source_root="$(port_integrations_root_path)"
    [ -d "$source_root" ] || error "Missing port integrations root: $source_root"

    mkdir -p "$target"
    cp -a "$source_root/." "$target/"
    stage_update_builder_resolved_port_integration_config "$update_builder_root"
    clear_update_builder_port_integration_config "$update_builder_root"
}

run_port_integration_package_hooks() {
    local staging_root="$1"
    local package_format="$2"
    local helper="$REPO_DIR/scripts/lib/port-integrations.js"
    local node_bin
    local integration_id
    local hook_path
    local hooks_output
    local app_dir="$staging_root/opt/$PACKAGE_NAME"

    [ -d "$staging_root" ] || error "Missing package staging root: $staging_root"
    [ -f "$helper" ] || error "Missing port integrations helper: $helper"

    node_bin="$(package_node_binary)"
    if ! hooks_output="$("$node_bin" "$helper" --package-hooks "$package_format")"; then
        error "Failed to discover port integration package hooks for $package_format"
    fi

    while IFS=$'\t' read -r integration_id hook_path; do
        [ -n "${integration_id:-}" ] || continue
        [ -f "$hook_path" ] || error "Missing port integration package hook for $integration_id: $hook_path"

        info "Running port integration package hook ($package_format): $integration_id"
        REPO_DIR="$REPO_DIR" \
            SCRIPT_DIR="$REPO_DIR" \
            APP_DIR="$app_dir" \
            PACKAGE_APP_DIR="$app_dir" \
            PACKAGE_NAME="$PACKAGE_NAME" \
            PACKAGE_VERSION="$PACKAGE_VERSION" \
            PACKAGE_FORMAT="$package_format" \
            PACKAGE_ROOT="$staging_root" \
            PACKAGE_STAGING_ROOT="$staging_root" \
            bash "$hook_path"
    done <<< "$hooks_output"
}

render_desktop_entry() {
    local target="$1"
    local package_name
    local display_name
    local comment
    local temp_target
    local filtered_target=""
    local temp_dir

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    display_name="$(sed_escape_replacement "${PACKAGE_DISPLAY_NAME:-Codex App}")"
    comment="$(sed_escape_replacement "${PACKAGE_COMMENT:-Run Codex App on Linux}")"
    temp_dir="$(dirname "$target")"
    temp_target="$(mktemp "$temp_dir/.${PACKAGE_NAME}.desktop.XXXXXX")" || \
        error "Failed to create temporary desktop entry"
    trap '[ -z "${temp_target:-}" ] || rm -f "$temp_target"; [ -z "${filtered_target:-}" ] || rm -f "$filtered_target"' RETURN

    sed \
        -e "s/codex-app-updater/__CODEX_APP_UPDATER__/g" \
        -e "s/codex-app/$package_name/g" \
        -e "s/__CODEX_APP_UPDATER__/codex-app-updater/g" \
        -e "0,/^Name=.*/s/^Name=.*/Name=$display_name/" \
        -e "0,/^Comment=.*/s/^Comment=.*/Comment=$comment/" \
        "$DESKTOP_TEMPLATE" > "$temp_target"
    if package_with_updater_enabled; then
        mv "$temp_target" "$target"
        temp_target=""
    else
        filtered_target="$(mktemp "$temp_dir/.${PACKAGE_NAME}.desktop.XXXXXX")" || \
            error "Failed to create filtered desktop entry"
        awk '
            /^Actions=/ {
                rendered = ""
                action_count = split(substr($0, 9), actions, ";")
                for (i = 1; i <= action_count; i++) {
                    if (actions[i] == "" ||
                        actions[i] == "CheckForUpdates" ||
                        actions[i] == "InstallReadyUpdate") {
                        continue
                    }
                    rendered = rendered actions[i] ";"
                }
                if (rendered != "") {
                    print "Actions=" rendered
                }
                next
            }
            /^\[Desktop Action CheckForUpdates\]$/ { skip = 1; next }
            /^\[Desktop Action InstallReadyUpdate\]$/ { skip = 1; next }
            /^\[/ { skip = 0 }
            skip { next }
            { print }
        ' "$temp_target" > "$filtered_target"
        mv "$filtered_target" "$target"
        filtered_target=""
        rm -f "$temp_target"
        temp_target=""
    fi
    trap - RETURN
    chmod 0644 "$target"
}

render_packaged_runtime_helper() {
    local target="$1"
    local package_name

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    sed \
        -e "s/CHROME_DESKTOP=\"codex-app.desktop\"/CHROME_DESKTOP=\"$package_name.desktop\"/" \
        -e "s|BAMF_DESKTOP_FILE_HINT=\"/usr/share/applications/codex-app.desktop\"|BAMF_DESKTOP_FILE_HINT=\"/usr/share/applications/$package_name.desktop\"|" \
        -e "s/__CODEX_PACKAGE_ENABLE_UPDATER__/$(package_with_updater_value)/g" \
        "$PACKAGED_RUNTIME_SOURCE" > "$target"
    chmod 0644 "$target"
}

render_no_updater_transition_cleanup_helper() {
    local target="$1"

    cat > "$target" <<'SCRIPT'
#!/bin/sh

SERVICE_NAME="${SERVICE_NAME:-codex-app-updater.service}"

codex_no_updater_foreach_user_manager() {
    if ! command -v runuser >/dev/null 2>&1 ||
        ! command -v systemctl >/dev/null 2>&1 ||
        ! command -v getent >/dev/null 2>&1; then
        return
    fi

    for runtime_dir in /run/user/*; do
        [ -d "$runtime_dir" ] || continue

        uid="$(basename "$runtime_dir")"
        case "$uid" in
            ''|*[!0-9]*|0)
                continue
                ;;
        esac

        bus="$runtime_dir/bus"
        [ -S "$bus" ] || continue

        user_name="$(getent passwd "$uid" | cut -d: -f1 || true)"
        [ -n "$user_name" ] || continue

        "$@" "$user_name" "$runtime_dir" "$bus"
    done
}

codex_no_updater_run_systemctl_user() {
    user_name="$1"
    runtime_dir="$2"
    bus="$3"
    shift 3

    runuser -u "$user_name" -- env \
        XDG_RUNTIME_DIR="$runtime_dir" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=$bus" \
        systemctl --user "$@" >/dev/null 2>&1
}

codex_no_updater_cleanup_one_user_manager() {
    user_name="$1"
    runtime_dir="$2"
    bus="$3"

    codex_no_updater_run_systemctl_user "$user_name" "$runtime_dir" "$bus" stop "$SERVICE_NAME" || true
    codex_no_updater_run_systemctl_user "$user_name" "$runtime_dir" "$bus" disable "$SERVICE_NAME" || true
    codex_no_updater_run_systemctl_user "$user_name" "$runtime_dir" "$bus" daemon-reload || true
}

codex_no_updater_cleanup_user_enablement_links() {
    if ! command -v getent >/dev/null 2>&1 || ! command -v runuser >/dev/null 2>&1; then
        return
    fi

    getent passwd | while IFS=: read -r user_name _ uid _ _ home _; do
        case "$uid" in
            ''|*[!0-9]*|0)
                continue
                ;;
        esac

        [ -n "$home" ] || continue
        [ "$home" != "/" ] || continue

        wants_dir="$home/.config/systemd/user/default.target.wants"
        service_link="$wants_dir/$SERVICE_NAME"
        [ -L "$service_link" ] || continue

        runuser -u "$user_name" -- rm -f "$service_link" >/dev/null 2>&1 || true
    done
}

codex_no_updater_cleanup_update_manager_service() {
    codex_no_updater_foreach_user_manager codex_no_updater_cleanup_one_user_manager
    codex_no_updater_cleanup_user_enablement_links
}
SCRIPT
    chmod 0644 "$target"
}

render_desktop_entry_doctor_helper() {
    local target="$1"

    cp "$REPO_DIR/packaging/linux/codex-app-desktop-entry-doctor.sh" "$target"
    chmod 0644 "$target"
}

write_no_updater_deb_postinst() {
    local target="$1"
    local package_name

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    cat > "$target" <<SCRIPT
#!/bin/sh
set -eu

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi

CLEANUP_HELPER="/usr/lib/$package_name/no-updater-transition-cleanup.sh"
DESKTOP_ENTRY_DOCTOR="/opt/$package_name/.codex-linux/codex-app-desktop-entry-doctor.sh"
if [ -f "\$CLEANUP_HELPER" ]; then
    # shellcheck source=/usr/lib/$package_name/no-updater-transition-cleanup.sh
    . "\$CLEANUP_HELPER"
    codex_no_updater_cleanup_update_manager_service || true
fi
if [ -f "\$DESKTOP_ENTRY_DOCTOR" ]; then
    # shellcheck source=/opt/$package_name/.codex-linux/codex-app-desktop-entry-doctor.sh
    . "\$DESKTOP_ENTRY_DOCTOR"
    codex_app_repair_system_package_shadow_entries $package_name || true
fi

exit 0
SCRIPT
    chmod 0755 "$target"
}

write_no_updater_deb_prerm() {
    local target="$1"
    local package_name

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    cat > "$target" <<SCRIPT
#!/bin/sh
set -eu

CLEANUP_HELPER="/usr/lib/$package_name/no-updater-transition-cleanup.sh"
if [ -f "\$CLEANUP_HELPER" ]; then
    # shellcheck source=/usr/lib/$package_name/no-updater-transition-cleanup.sh
    . "\$CLEANUP_HELPER"
    codex_no_updater_cleanup_update_manager_service || true
fi

exit 0
SCRIPT
    chmod 0755 "$target"
}

write_no_updater_deb_postrm() {
    local target="$1"
    local package_name

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    cat > "$target" <<SCRIPT
#!/bin/sh
set -eu

CLEANUP_HELPER="/usr/lib/$package_name/no-updater-transition-cleanup.sh"
if [ -f "\$CLEANUP_HELPER" ]; then
    # shellcheck source=/usr/lib/$package_name/no-updater-transition-cleanup.sh
    . "\$CLEANUP_HELPER"
    codex_no_updater_cleanup_update_manager_service || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
fi

exit 0
SCRIPT
    chmod 0755 "$target"
}

write_no_updater_pacman_install_hooks() {
    local target="$1"
    local package_name

    package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
    cat > "$target" <<SCRIPT
CLEANUP_HELPER="/usr/lib/$package_name/no-updater-transition-cleanup.sh"
DESKTOP_ENTRY_DOCTOR="/opt/$package_name/.codex-linux/codex-app-desktop-entry-doctor.sh"

codex_no_updater_cleanup_if_present() {
    if [ -f "\$CLEANUP_HELPER" ]; then
        # shellcheck source=/usr/lib/$package_name/no-updater-transition-cleanup.sh
        . "\$CLEANUP_HELPER"
        codex_no_updater_cleanup_update_manager_service || true
    fi
}

codex_app_repair_if_present() {
    if [ -f "\$DESKTOP_ENTRY_DOCTOR" ]; then
        # shellcheck source=/opt/$package_name/.codex-linux/codex-app-desktop-entry-doctor.sh
        . "\$DESKTOP_ENTRY_DOCTOR"
        codex_app_repair_system_package_shadow_entries $package_name || true
    fi
}

post_install() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
    fi
    codex_app_repair_if_present
    codex_no_updater_cleanup_if_present
}

post_upgrade() {
    post_install
}

pre_remove() {
    codex_no_updater_cleanup_if_present
}
SCRIPT
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

find_cargo_command() {
    if command -v cargo >/dev/null 2>&1; then
        command -v cargo
        return 0
    fi

    if [ -n "${HOME-}" ] && [ -x "$HOME/.cargo/bin/cargo" ]; then
        echo "$HOME/.cargo/bin/cargo"
        return 0
    fi

    return 1
}

ensure_updater_binary() {
    local cargo_cmd=""

    if ! package_with_updater_enabled; then
        return
    fi

    if [ -x "$UPDATER_BINARY_SOURCE" ] && ! updater_binary_is_stale "$UPDATER_BINARY_SOURCE"; then
        return
    fi

    [ -f "$REPO_DIR/Cargo.toml" ] || error "Missing updater binary: $UPDATER_BINARY_SOURCE"
    cargo_cmd="$(find_cargo_command)" || error "cargo is required to build codex-app-updater.
Install the Rust toolchain:
  bash scripts/install-deps.sh        # auto-installs via rustup
  # or manually: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"

    info "Building codex-app-updater release binary"
    "$cargo_cmd" build --release -p codex-app-updater >&2
    [ -x "$UPDATER_BINARY_SOURCE" ] || error "Failed to build updater binary: $UPDATER_BINARY_SOURCE"
}

stage_update_builder_source_info() {
    local update_builder_root="$1"
    local info_dir="$update_builder_root/.codex-linux"
    local info_file="$info_dir/source-info.json"
    local node_bin

    mkdir -p "$info_dir"
    node_bin="$(package_node_binary)"
    "$node_bin" - "$REPO_DIR" "$info_file" <<'NODE'
const childProcess = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const [repoDir, infoFile] = process.argv.slice(2);

function git(args) {
  const result = childProcess.spawnSync("git", ["-C", repoDir, ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  if (result.status !== 0) {
    return null;
  }
  const value = result.stdout.trim();
  return value.length > 0 ? value : null;
}

function isoTimestamp() {
  const rawEpoch = process.env.SOURCE_DATE_EPOCH?.trim();
  if (rawEpoch) {
    const epochSeconds = Number(rawEpoch);
    if (Number.isFinite(epochSeconds) && epochSeconds >= 0) {
      return new Date(Math.trunc(epochSeconds) * 1000).toISOString();
    }
  }
  return new Date().toISOString();
}

function sanitizeGitRemoteUrl(remote) {
  if (remote == null) {
    return null;
  }
  const value = String(remote).trim();
  if (
    value.length === 0
    || path.isAbsolute(value)
    || path.win32.isAbsolute(value)
    || value === "."
    || value === ".."
    || value.startsWith("./")
    || value.startsWith("../")
    || value === "~"
    || value.startsWith("~/")
  ) {
    return null;
  }
  try {
    const url = new URL(value);
    if (url.protocol === "file:") {
      return null;
    }
    if (url.username || url.password) {
      url.username = "";
      url.password = "";
      return url.toString();
    }
  } catch {
    if (/^(?:[^@\s/:]+@)?[^@\s/:]+:.+$/.test(value)) {
      return value;
    }
    return null;
  }
  return value;
}

function shortSourceCommit(commit) {
  if (commit == null) {
    return null;
  }
  const value = String(commit);
  const suffix = value.endsWith("-dirty") ? "-dirty" : "";
  const revision = suffix ? value.slice(0, -suffix.length) : value;
  return `${revision.slice(0, 12)}${suffix}`;
}

function readJsonFile(filePath) {
  try {
    const value = JSON.parse(fs.readFileSync(filePath, "utf8"));
    return value != null && typeof value === "object" && !Array.isArray(value) ? value : null;
  } catch {
    return null;
  }
}

function parseWrapperVersion(content) {
  let inPackage = false;
  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      inPackage = trimmed === "[package]";
      continue;
    }
    if (!inPackage) {
      continue;
    }
    const match = trimmed.match(/^version\s*=\s*"([^"]+)"/);
    if (match) {
      return match[1];
    }
  }
  return null;
}

function readWrapperVersion(repoDir) {
  try {
    return parseWrapperVersion(fs.readFileSync(path.join(repoDir, "updater", "Cargo.toml"), "utf8"));
  } catch {
    return null;
  }
}

function sanitizeSourceInfo(info) {
  return {
    ...info,
    version: info.version ?? readWrapperVersion(repoDir),
    remote: sanitizeGitRemoteUrl(info.remote),
    provenance: info.provenance ?? "packaged-update-builder",
    recapturedAt: isoTimestamp(),
  };
}

const stagedInfo = readJsonFile(path.join(repoDir, ".codex-linux", "source-info.json"));
const commit = process.env.CODEX_LINUX_SOURCE_COMMIT?.trim() || git(["rev-parse", "HEAD"]);
const status = git(["status", "--porcelain"]);
const info = stagedInfo?.commit
  ? sanitizeSourceInfo(stagedInfo)
  : {
      commit,
      shortCommit: shortSourceCommit(commit),
      version: readWrapperVersion(repoDir),
      branch: process.env.CODEX_LINUX_SOURCE_BRANCH?.trim() || git(["branch", "--show-current"]),
      remote: sanitizeGitRemoteUrl(process.env.CODEX_LINUX_SOURCE_REMOTE?.trim() || git(["remote", "get-url", "origin"])),
      describe: process.env.CODEX_LINUX_SOURCE_DESCRIBE?.trim() || git(["describe", "--always", "--dirty", "--tags"]),
      dirty: status == null ? null : status.length > 0,
      provenance: "packaged-update-builder",
      capturedAt: isoTimestamp(),
    };

fs.mkdirSync(path.dirname(infoFile), { recursive: true });
fs.writeFileSync(infoFile, `${JSON.stringify(info, null, 2)}\n`, "utf8");
NODE
}

stage_common_package_files() {
    local root="$1"
    local app_root="$root/opt/$PACKAGE_NAME"
    local support_root="$root/usr/lib/$PACKAGE_NAME"
    local polkit_policy="$REPO_DIR/packaging/linux/com.github.nisavid.codex-app.update.policy"

    validate_package_inputs
    validate_app_payload_source
    if package_with_updater_enabled; then
        ensure_file_exists "$polkit_policy" "polkit policy"
    fi

    mkdir -p \
        "$root/opt" \
        "$root/usr/bin" \
        "$support_root" \
        "$root/usr/share/applications" \
        "$root/usr/share/icons/hicolor/256x256/apps"

    rm -rf "$app_root"
    cp -aT "$APP_DIR" "$app_root"
    normalize_app_payload_modes "$app_root"
    mkdir -p "$app_root/.codex-linux"
    cp "$ICON_SOURCE" "$app_root/.codex-linux/$PACKAGE_NAME.png"
    render_desktop_entry_doctor_helper "$app_root/.codex-linux/codex-app-desktop-entry-doctor.sh"
    render_desktop_entry "$root/usr/share/applications/$PACKAGE_NAME.desktop"
    cp "$ICON_SOURCE" "$root/usr/share/icons/hicolor/256x256/apps/$PACKAGE_NAME.png"
    if package_with_updater_enabled; then
        mkdir -p \
            "$root/usr/lib/systemd/user" \
            "$root/usr/share/polkit-1/actions"
        cp "$UPDATER_BINARY_SOURCE" "$root/usr/bin/codex-app-updater"
        chmod 0755 "$root/usr/bin/codex-app-updater"
        cp "$UPDATER_SERVICE_SOURCE" "$root/usr/lib/systemd/user/codex-app-updater.service"
        chmod 0644 "$root/usr/lib/systemd/user/codex-app-updater.service"
        cp "$polkit_policy" "$root/usr/share/polkit-1/actions/com.github.nisavid.codex-app.update.policy"
        chmod 0644 "$root/usr/share/polkit-1/actions/com.github.nisavid.codex-app.update.policy"
    else
        render_no_updater_transition_cleanup_helper \
            "$support_root/no-updater-transition-cleanup.sh"
    fi
    render_packaged_runtime_helper "$support_root/packaged-runtime.sh"
}

stage_update_builder_bundle() {
    package_with_updater_enabled || return 0

    local root="$1"
    local update_builder_root="$root/usr/lib/$PACKAGE_NAME/update-builder"
    local node_runtime_source="$APP_DIR/resources/node-runtime"

    mkdir -p \
        "$update_builder_root/scripts" \
        "$update_builder_root/scripts/lib" \
        "$update_builder_root/scripts/patches" \
        "$update_builder_root/launcher" \
        "$update_builder_root/port-integrations" \
        "$update_builder_root/packaging/linux" \
        "$update_builder_root/assets"

    cp "$REPO_DIR/install.sh" "$update_builder_root/install.sh"
    cp "$REPO_DIR/CHANGELOG.md" "$update_builder_root/CHANGELOG.md"
    cp "$REPO_DIR/launcher/start.sh.template" "$update_builder_root/launcher/start.sh.template"
    cp "$REPO_DIR/launcher/webview-server.py" "$update_builder_root/launcher/webview-server.py"
    cp "$REPO_DIR/Cargo.toml" "$update_builder_root/Cargo.toml"
    cp "$REPO_DIR/Cargo.lock" "$update_builder_root/Cargo.lock"
    cp -r "$REPO_DIR/computer-use-linux" "$update_builder_root/computer-use-linux"
    cp -r "$REPO_DIR/read-aloud-linux" "$update_builder_root/read-aloud-linux"
    cp -r "$REPO_DIR/updater" "$update_builder_root/updater"
    mkdir -p "$update_builder_root/plugins/openai-bundled/plugins"
    cp -r "$REPO_DIR/plugins/openai-bundled/plugins/computer-use" \
        "$update_builder_root/plugins/openai-bundled/plugins/computer-use"
    cp -r "$REPO_DIR/plugins/openai-bundled/plugins/read-aloud" \
        "$update_builder_root/plugins/openai-bundled/plugins/read-aloud"
    cp "$REPO_DIR/scripts/build-deb.sh" "$update_builder_root/scripts/build-deb.sh"
    cp "$REPO_DIR/scripts/build-rpm.sh" "$update_builder_root/scripts/build-rpm.sh"
    cp "$REPO_DIR/scripts/build-pacman.sh" "$update_builder_root/scripts/build-pacman.sh"
    cp "$REPO_DIR/scripts/rebuild-candidate.sh" "$update_builder_root/scripts/rebuild-candidate.sh"
    cp "$REPO_DIR/scripts/patch-linux-window-ui.js" "$update_builder_root/scripts/patch-linux-window-ui.js"
    cp -r "$REPO_DIR/scripts/patches/." "$update_builder_root/scripts/patches/"
    cp "$REPO_DIR/scripts/lib/package-common.sh" "$update_builder_root/scripts/lib/package-common.sh"
    cp "$REPO_DIR/scripts/lib/patch-chrome-plugin.js" "$update_builder_root/scripts/lib/patch-chrome-plugin.js"
    cp "$REPO_DIR/scripts/lib/node-runtime.sh" "$update_builder_root/scripts/lib/node-runtime.sh"
    cp "$REPO_DIR/scripts/lib/install-helpers.sh" "$update_builder_root/scripts/lib/install-helpers.sh"
    cp "$REPO_DIR/scripts/lib/process-detection.sh" "$update_builder_root/scripts/lib/process-detection.sh"
    cp "$REPO_DIR/scripts/lib/dmg.sh" "$update_builder_root/scripts/lib/dmg.sh"
    cp "$REPO_DIR/scripts/lib/native-modules.sh" "$update_builder_root/scripts/lib/native-modules.sh"
    cp "$REPO_DIR/scripts/lib/asar-patch.sh" "$update_builder_root/scripts/lib/asar-patch.sh"
    cp "$REPO_DIR/scripts/lib/webview-install.sh" "$update_builder_root/scripts/lib/webview-install.sh"
    cp "$REPO_DIR/scripts/lib/bundled-plugins.sh" "$update_builder_root/scripts/lib/bundled-plugins.sh"
    cp "$REPO_DIR/scripts/lib/port-integrations.js" "$update_builder_root/scripts/lib/port-integrations.js"
    cp "$REPO_DIR/scripts/lib/port-integrations.sh" "$update_builder_root/scripts/lib/port-integrations.sh"
    cp "$REPO_DIR/scripts/lib/linux-target-context.js" "$update_builder_root/scripts/lib/linux-target-context.js"
    cp "$REPO_DIR/scripts/lib/linux-update-bridge-patch.js" "$update_builder_root/scripts/lib/linux-update-bridge-patch.js"
    cp "$REPO_DIR/scripts/lib/patch-report.js" "$update_builder_root/scripts/lib/patch-report.js"
    cp "$REPO_DIR/scripts/lib/rebuild-report.sh" "$update_builder_root/scripts/lib/rebuild-report.sh"
    cp "$REPO_DIR/scripts/lib/build-info.js" "$update_builder_root/scripts/lib/build-info.js"
    cp "$REPO_DIR/scripts/lib/build-info.sh" "$update_builder_root/scripts/lib/build-info.sh"
    cp "$REPO_DIR/packaging/linux/control" "$update_builder_root/packaging/linux/control"
    cp "$REPO_DIR/packaging/linux/codex-app.spec" "$update_builder_root/packaging/linux/codex-app.spec"
    cp "$REPO_DIR/packaging/linux/codex-app.desktop" "$update_builder_root/packaging/linux/codex-app.desktop"
    cp "$REPO_DIR/packaging/linux/codex-app-desktop-entry-doctor.sh" \
        "$update_builder_root/packaging/linux/codex-app-desktop-entry-doctor.sh"
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
    stage_update_builder_port_integrations_tree "$update_builder_root"
    cp "$REPO_DIR/packaging/linux/codex-app-updater.postrm" "$update_builder_root/packaging/linux/codex-app-updater.postrm"
    cp "$REPO_DIR/assets/codex.png" "$update_builder_root/assets/codex.png"
    stage_update_builder_source_info "$update_builder_root"
    if [ -d "$node_runtime_source" ]; then
        cp -a "$node_runtime_source" "$update_builder_root/node-runtime"
    else
        error "Missing managed Node.js runtime: $node_runtime_source. Run ./install.sh first."
    fi
}

stage_optional_update_builder_bundle() {
    if package_with_updater_enabled; then
        stage_update_builder_bundle "$@"
    else
        info "Skipping update-builder bundle (PACKAGE_WITH_UPDATER=0)"
    fi
}

restore_port_integration_payload_permissions() {
    local root="$1"
    local helper="$REPO_DIR/scripts/lib/port-integrations.js"
    local app_root="$root/opt/$PACKAGE_NAME"
    local node_bin
    local staged_files_json

    [ -d "$root" ] || error "Missing package root: $root"
    [ -d "$app_root" ] || error "Missing package app root: $app_root"
    [ -f "$helper" ] || error "Missing port integrations helper: $helper"

    node_bin="$(package_node_binary)"
    if ! staged_files_json="$("$node_bin" "$helper" --staged-files-json "$app_root")"; then
        error "Failed to read port integration staged file manifest"
    fi

    if ! "$node_bin" - "$app_root" "$staged_files_json" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

const [appRoot, rawJson] = process.argv.slice(2);
const entries = JSON.parse(rawJson);

if (!Array.isArray(entries)) {
  throw new Error("port integration staged files payload must be an array");
}

function assertRelativeTarget(target) {
  if (typeof target !== "string" || target.length === 0) {
    throw new Error("port integration staged file target must be a relative path");
  }
  const parts = target.split(/[\\/]+/).filter(Boolean);
  if (path.isAbsolute(target) || parts.includes("..")) {
    throw new Error(`Unsafe port integration staged file target: ${target}`);
  }
  const resolved = path.resolve(appRoot, ...parts);
  const relative = path.relative(appRoot, resolved);
  if (relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Unsafe port integration staged file target: ${target}`);
  }
  return resolved;
}

for (const entry of entries) {
  if (entry == null || typeof entry !== "object") {
    throw new Error("port integration staged file entry must be an object");
  }
  if (typeof entry.mode !== "string" || !/^[0-7]{3,4}$/.test(entry.mode)) {
    throw new Error(`Invalid port integration staged file mode for ${entry.target}: ${entry.mode}`);
  }
  const target = assertRelativeTarget(entry.target);
  if (!fs.existsSync(target)) {
    throw new Error(`port integration staged file is missing from package payload: ${entry.target}`);
  }
  fs.chmodSync(target, Number.parseInt(entry.mode, 8));
}
NODE
    then
        error "Failed to restore port integration staged file permissions"
    fi
}

normalize_package_payload_permissions() {
    local root="$1"

    [ -d "$root" ] || error "Missing package root: $root"
    find "$root" -type d -exec chmod 0755 {} +
    find "$root" -type f \( -perm /u=x -o -perm /g=x -o -perm /o=x \) -exec chmod 0755 {} +
    find "$root" -type f ! \( -perm /u=x -o -perm /g=x -o -perm /o=x \) -exec chmod 0644 {} +
}

write_launcher_stub() {
    local root="$1"

    cat > "$root/usr/bin/$PACKAGE_NAME" <<SCRIPT
#!/bin/bash
exec /opt/$PACKAGE_NAME/start.sh "\$@"
SCRIPT
    chmod 0755 "$root/usr/bin/$PACKAGE_NAME"
}
