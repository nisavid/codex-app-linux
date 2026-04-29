#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TMP_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

info() {
    echo "[smoke] $*" >&2
}

fail() {
    echo "[smoke][FAIL] $*" >&2
    exit 1
}

assert_file_exists() {
    local path="$1"
    [ -f "$path" ] || fail "Expected file to exist: $path"
}

assert_contains() {
    local path="$1"
    local pattern="$2"
    grep -q -- "$pattern" "$path" || fail "Expected '$pattern' in $path"
}

assert_not_contains() {
    local path="$1"
    local pattern="$2"
    if grep -q -- "$pattern" "$path"; then
        fail "Did not expect '$pattern' in $path"
    fi
}

assert_occurrence_count() {
    local path="$1"
    local pattern="$2"
    local expected="$3"
    local actual
    actual="$(grep -o -- "$pattern" "$path" | wc -l | tr -d ' ')"
    [ "$actual" = "$expected" ] || fail "Expected '$pattern' to appear $expected times in $path, found $actual"
}

make_fake_app() {
    local app_dir="$1"
    mkdir -p "$app_dir"
    cat > "$app_dir/start.sh" <<'SCRIPT'
#!/bin/bash
exit 0
SCRIPT
    chmod +x "$app_dir/start.sh"
    cat > "$app_dir/codex-app-version.env" <<'EOF'
CODEX_APP_UPSTREAM_VERSION=26.422.30944
CODEX_APP_UPSTREAM_BUILD=2080
CODEX_APP_PACKAGE_VERSION=26.422.30944.2080
EOF
}

make_stub_bin_dir() {
    local bin_dir="$1"
    mkdir -p "$bin_dir"
}

test_common_helper_sourcing() {
    info "Checking shared packaging helpers"
    local probe_file="$TMP_DIR/probe.txt"
    touch "$probe_file"

    # shellcheck disable=SC1091
    source "$REPO_DIR/scripts/lib/package-common.sh"
    ensure_file_exists "$probe_file" "probe file"
}

test_package_version_metadata_is_read_as_data() {
    info "Checking package version metadata is not sourced"
    local workspace="$TMP_DIR/package-version-data"
    local app_dir="$workspace/app"
    local marker="$workspace/sourced-marker"

    mkdir -p "$app_dir"
    cat > "$app_dir/codex-app-version.env" <<EOF
CODEX_APP_PACKAGE_VERSION=\$(touch "$marker"; printf 26.422.30944.2080)
EOF

    if (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        APP_DIR="$app_dir" PACKAGE_VERSION="" resolve_package_version >/dev/null 2>&1
    ); then
        fail "Expected malicious package version metadata to be rejected"
    fi
    [ ! -e "$marker" ] || fail "Package version metadata was executed as shell code"
}

test_package_version_metadata_trims_trailing_whitespace() {
    info "Checking package version metadata trimming"
    local workspace="$TMP_DIR/package-version-trim"
    local app_dir="$workspace/app"
    local version

    mkdir -p "$app_dir"
    printf '# comment\r\n\r\nCODEX_APP_PACKAGE_VERSION=26.422.30944.2080   \r\n' > "$app_dir/codex-app-version.env"

    version="$(
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        APP_DIR="$app_dir" PACKAGE_VERSION="" resolve_package_version
    )"
    [ "$version" = "26.422.30944.2080" ] || fail "Expected trimmed package version, got: $version"
}

test_package_version_metadata_rejects_alphanumeric_segments() {
    info "Checking package version metadata rejects alphanumeric segments"
    local workspace="$TMP_DIR/package-version-alpha"
    local app_dir="$workspace/app"

    mkdir -p "$app_dir"
    printf 'CODEX_APP_PACKAGE_VERSION=26.422.30944b.2080\n' > "$app_dir/codex-app-version.env"

    if (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        APP_DIR="$app_dir" PACKAGE_VERSION="" resolve_package_version >/dev/null 2>&1
    ); then
        fail "Expected alphanumeric package version metadata to be rejected"
    fi
}

test_package_version_metadata_rejects_too_few_segments() {
    info "Checking package version metadata rejects too few segments"
    local workspace="$TMP_DIR/package-version-short"
    local app_dir="$workspace/app"

    mkdir -p "$app_dir"
    printf 'CODEX_APP_PACKAGE_VERSION=26.422\n' > "$app_dir/codex-app-version.env"

    if (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        APP_DIR="$app_dir" PACKAGE_VERSION="" resolve_package_version >/dev/null 2>&1
    ); then
        fail "Expected short package version metadata to be rejected"
    fi
}

test_package_identifiers_reject_path_characters() {
    info "Checking package identifier validation rejects path characters"
    (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        PACKAGE_NAME="codex-app" \
        PACKAGE_PROVIDES="codex-desktop" \
        PACKAGE_CONFLICTS="codex-desktop" \
        APP_INSTALL_NAME="codex-app" \
        APP_LAUNCHER_NAME="codex-app" \
        validate_packaging_identifiers
    )

    if (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        PACKAGE_NAME="../codex-app" \
        PACKAGE_PROVIDES="codex-desktop" \
        PACKAGE_CONFLICTS="codex-desktop" \
        APP_INSTALL_NAME="codex-app" \
        APP_LAUNCHER_NAME="codex-app" \
        validate_packaging_identifiers >/dev/null 2>&1
    ); then
        fail "Expected package identifier validation to reject path characters"
    fi
}

test_package_staging_rejects_unsafe_symlinks() {
    info "Checking package staging rejects unsafe symlinks"
    local workspace="$TMP_DIR/package-unsafe-symlink"
    local app_dir="$workspace/app"
    local root="$workspace/root"
    local updater_bin="$workspace/codex-app-updater"

    make_fake_app "$app_dir"
    ln -s /etc/passwd "$app_dir/unsafe-absolute"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    if (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        PACKAGE_NAME=codex-app \
        APP_DIR="$app_dir" \
        DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop" \
        ICON_SOURCE="$REPO_DIR/assets/codex.png" \
        UPDATER_BINARY_SOURCE="$updater_bin" \
        UPDATER_SERVICE_SOURCE="$REPO_DIR/packaging/linux/codex-app-updater.service" \
        PACKAGED_RUNTIME_SOURCE="$REPO_DIR/packaging/linux/packaged-runtime.sh" \
        stage_common_package_files "$root" >/dev/null 2>&1
    ); then
        fail "Expected package staging to reject absolute symlink"
    fi
}

test_package_staging_normalizes_payload_modes() {
    info "Checking package staging normalizes app payload modes"
    local workspace="$TMP_DIR/package-normalize-modes"
    local app_dir="$workspace/app"
    local root="$workspace/root"
    local updater_bin="$workspace/codex-app-updater"
    local staged_file="$root/opt/codex-app/world-writable.txt"
    local staged_exec="$root/opt/codex-app/setuid-helper"

    make_fake_app "$app_dir"
    printf 'data\n' > "$app_dir/world-writable.txt"
    chmod 0666 "$app_dir/world-writable.txt"
    printf '#!/bin/bash\nexit 0\n' > "$app_dir/setuid-helper"
    chmod 6755 "$app_dir/setuid-helper"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    (
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        PACKAGE_NAME=codex-app \
        APP_DIR="$app_dir" \
        DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop" \
        ICON_SOURCE="$REPO_DIR/assets/codex.png" \
        UPDATER_BINARY_SOURCE="$updater_bin" \
        UPDATER_SERVICE_SOURCE="$REPO_DIR/packaging/linux/codex-app-updater.service" \
        PACKAGED_RUNTIME_SOURCE="$REPO_DIR/packaging/linux/packaged-runtime.sh" \
        stage_common_package_files "$root"
    )

    [ "$(stat -c '%a' "$staged_file")" = "644" ] || fail "Expected staged regular file mode 644"
    [ "$(stat -c '%a' "$staged_exec")" = "755" ] || fail "Expected staged executable mode 755"
}

test_package_staging_normalizes_package_file_modes_under_private_umask() {
    info "Checking package staging normalizes package file modes under private umask"
    local workspace="$TMP_DIR/package-root-file-modes"
    local app_dir="$workspace/app"
    local root="$workspace/root"
    local updater_bin="$workspace/codex-app-updater"

    make_fake_app "$app_dir"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    (
        umask 077
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        PACKAGE_NAME=codex-app \
        APP_DIR="$app_dir" \
        DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop" \
        ICON_SOURCE="$REPO_DIR/assets/codex.png" \
        UPDATER_BINARY_SOURCE="$updater_bin" \
        UPDATER_SERVICE_SOURCE="$REPO_DIR/packaging/linux/codex-app-updater.service" \
        PACKAGED_RUNTIME_SOURCE="$REPO_DIR/packaging/linux/packaged-runtime.sh" \
        stage_common_package_files "$root"
        PACKAGE_NAME=codex-app \
        UPDATER_SERVICE_SOURCE="$REPO_DIR/packaging/linux/codex-app-updater.service" \
        stage_update_builder_bundle "$root"
        PACKAGE_NAME=codex-app write_launcher_stub "$root"
    )

    [ "$(stat -c '%a' "$root/usr/lib/codex-app/update-builder/install.sh")" = "755" ] || fail "Expected update-builder install.sh mode 755"
    [ "$(stat -c '%a' "$root/usr/lib/codex-app/update-builder/scripts/lib/package-common.sh")" = "755" ] || fail "Expected update-builder package-common.sh mode 755"
}

test_package_staging_normalizes_system_directory_modes_under_private_umask() {
    info "Checking package staging normalizes system directory modes under private umask"
    local workspace="$TMP_DIR/package-system-dir-modes"
    local app_dir="$workspace/app"
    local root="$workspace/root"
    local updater_bin="$workspace/codex-app-updater"

    make_fake_app "$app_dir"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    (
        umask 077
        # shellcheck disable=SC1091
        source "$REPO_DIR/scripts/lib/package-common.sh"
        PACKAGE_NAME=codex-app \
        APP_DIR="$app_dir" \
        DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop" \
        ICON_SOURCE="$REPO_DIR/assets/codex.png" \
        UPDATER_BINARY_SOURCE="$updater_bin" \
        UPDATER_SERVICE_SOURCE="$REPO_DIR/packaging/linux/codex-app-updater.service" \
        PACKAGED_RUNTIME_SOURCE="$REPO_DIR/packaging/linux/packaged-runtime.sh" \
        stage_common_package_files "$root"
        PACKAGE_NAME=codex-app \
        UPDATER_SERVICE_SOURCE="$REPO_DIR/packaging/linux/codex-app-updater.service" \
        stage_update_builder_bundle "$root"
        PACKAGE_NAME=codex-app write_launcher_stub "$root"
    )

    for dir in \
        "$root/opt" \
        "$root/opt/codex-app" \
        "$root/usr" \
        "$root/usr/bin" \
        "$root/usr/lib" \
        "$root/usr/lib/codex-app" \
        "$root/usr/lib/codex-app/update-builder" \
        "$root/usr/share" \
        "$root/usr/share/applications"; do
        [ "$(stat -c '%a' "$dir")" = "755" ] || fail "Expected directory mode 755 for $dir"
    done
}

test_deb_builder_smoke() {
    info "Running Debian packaging smoke test"
    local workspace="$TMP_DIR/deb"
    local bin_dir="$workspace/bin"
    local app_dir="$workspace/app"
    local dist_dir="$workspace/dist"
    local pkg_root="$workspace/deb-root"
    local updater_bin="$workspace/codex-app-updater"

    mkdir -p "$workspace" "$dist_dir"
    make_stub_bin_dir "$bin_dir"
    make_fake_app "$app_dir"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    cat > "$bin_dir/dpkg" <<'SCRIPT'
#!/bin/bash
if [ "$1" = "--print-architecture" ]; then
    echo amd64
    exit 0
fi
exit 0
SCRIPT
    cat > "$bin_dir/dpkg-deb" <<'SCRIPT'
#!/bin/bash
output="${@: -1}"
mkdir -p "$(dirname "$output")"
touch "$output"
SCRIPT
    cat > "$bin_dir/cargo" <<'SCRIPT'
#!/bin/bash
echo "cargo should not be called when UPDATER_BINARY_SOURCE exists" >&2
exit 99
SCRIPT
    chmod +x "$bin_dir/dpkg" "$bin_dir/dpkg-deb" "$bin_dir/cargo"

    PATH="$bin_dir:$PATH" \
    APP_DIR_OVERRIDE="$app_dir" \
    PKG_ROOT_OVERRIDE="$pkg_root" \
    DIST_DIR_OVERRIDE="$dist_dir" \
    UPDATER_BINARY_SOURCE="$updater_bin" \
    "$REPO_DIR/scripts/build-deb.sh"

    assert_file_exists "$dist_dir/codex-app_26.422.30944.2080_amd64.deb"
    assert_file_exists "$pkg_root/DEBIAN/prerm"
    assert_file_exists "$pkg_root/DEBIAN/postrm"
    assert_file_exists "$pkg_root/usr/lib/codex-app/update-builder/scripts/lib/package-common.sh"
    assert_file_exists "$pkg_root/usr/lib/codex-app/packaged-runtime.sh"
}

test_rpm_builder_smoke() {
    info "Running RPM packaging smoke test"
    local workspace="$TMP_DIR/rpm"
    local bin_dir="$workspace/bin"
    local app_dir="$workspace/app"
    local dist_dir="$workspace/dist"
    local updater_bin="$workspace/codex-app-updater"

    mkdir -p "$workspace" "$dist_dir"
    make_stub_bin_dir "$bin_dir"
    make_fake_app "$app_dir"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    cat > "$bin_dir/rpmbuild" <<'SCRIPT'
#!/bin/bash
rpmdir=""
while [ $# -gt 0 ]; do
    if [ "$1" = "--define" ]; then
        case "$2" in
            _rpmdir\ *) rpmdir="${2#_rpmdir }" ;;
        esac
        shift 2
        continue
    fi
    shift
done
[ -n "$rpmdir" ] || exit 1
mkdir -p "$rpmdir/x86_64"
touch "$rpmdir/x86_64/codex-app-26.422.30944.2080-1.x86_64.rpm"
SCRIPT
    cat > "$bin_dir/cargo" <<'SCRIPT'
#!/bin/bash
echo "cargo should not be called when UPDATER_BINARY_SOURCE exists" >&2
exit 99
SCRIPT
    chmod +x "$bin_dir/rpmbuild" "$bin_dir/cargo"

    PATH="$bin_dir:$PATH" \
    APP_DIR_OVERRIDE="$app_dir" \
    DIST_DIR_OVERRIDE="$dist_dir" \
    UPDATER_BINARY_SOURCE="$updater_bin" \
    TEST_RPM_STAGING="$workspace/staging" \
    "$REPO_DIR/scripts/build-rpm.sh"

    assert_file_exists "$dist_dir/codex-app-26.422.30944.2080-1.x86_64.rpm"
    assert_file_exists "$workspace/staging/usr/lib/codex-app/update-builder/scripts/lib/package-common.sh"
    assert_file_exists "$workspace/staging/usr/lib/codex-app/update-builder/scripts/patch-linux-window-ui.js"
}

test_pacman_builder_metadata_smoke() {
    info "Running pacman packaging metadata smoke test"
    local workspace="$TMP_DIR/pacman"
    local bin_dir="$workspace/bin"
    local app_dir="$workspace/app"
    local dist_dir="$workspace/dist"
    local updater_bin="$workspace/codex-app-updater"
    local captured_pkgbuild="$workspace/PKGBUILD.rendered"

    mkdir -p "$workspace" "$dist_dir"
    make_stub_bin_dir "$bin_dir"
    make_fake_app "$app_dir"
    printf '#!/bin/bash\nexit 0\n' > "$updater_bin"
    chmod +x "$updater_bin"

    cat > "$bin_dir/makepkg" <<'SCRIPT'
#!/bin/bash
cp PKGBUILD "$TEST_PACMAN_CAPTURE"
grep -q 'pkgname=codex-app' PKGBUILD || exit 11
grep -q "provides=('codex-desktop')" PKGBUILD || exit 12
grep -q "conflicts=('codex-desktop')" PKGBUILD || exit 13
grep -q "install=codex-app.install" PKGBUILD || exit 14
grep -q 'cp -a --no-preserve=ownership' PKGBUILD || exit 15
test -x "$TEST_PACMAN_STAGING/usr/bin/codex-app" || exit 16
test -d "$TEST_PACMAN_STAGING/opt/codex-app" || exit 17
test -f "$TEST_PACMAN_STAGING/usr/lib/codex-app/packaged-runtime.sh" || exit 18
touch "$PKGDEST/codex-app-26.422.30944.2080-1-x86_64.pkg.tar.zst"
SCRIPT
    cat > "$bin_dir/cargo" <<'SCRIPT'
#!/bin/bash
echo "cargo should not be called when UPDATER_BINARY_SOURCE exists" >&2
exit 99
SCRIPT
    chmod +x "$bin_dir/makepkg" "$bin_dir/cargo"

    PATH="$bin_dir:$PATH" \
    APP_DIR_OVERRIDE="$app_dir" \
    DIST_DIR_OVERRIDE="$dist_dir" \
    UPDATER_BINARY_SOURCE="$updater_bin" \
    TEST_PACMAN_CAPTURE="$captured_pkgbuild" \
    TEST_PACMAN_STAGING="$workspace/staging" \
    "$REPO_DIR/scripts/build-pacman.sh"

    assert_file_exists "$dist_dir/codex-app-26.422.30944.2080-1-x86_64.pkg.tar.zst"
    assert_contains "$captured_pkgbuild" "pkgname=codex-app"
    assert_contains "$captured_pkgbuild" "provides=('codex-desktop')"
    assert_contains "$captured_pkgbuild" "conflicts=('codex-desktop')"
    assert_contains "$captured_pkgbuild" "install=codex-app.install"
    assert_contains "$captured_pkgbuild" 'cp -a --no-preserve=ownership'
    assert_file_exists "$workspace/staging/usr/bin/codex-app"
    assert_file_exists "$workspace/staging/usr/lib/codex-app/packaged-runtime.sh"
}

test_missing_input_failure() {
    info "Checking missing-input failure path"
    local workspace="$TMP_DIR/missing"
    local bin_dir="$workspace/bin"

    mkdir -p "$workspace"
    make_stub_bin_dir "$bin_dir"
    cat > "$bin_dir/dpkg" <<'SCRIPT'
#!/bin/bash
echo amd64
SCRIPT
    cat > "$bin_dir/dpkg-deb" <<'SCRIPT'
#!/bin/bash
exit 0
SCRIPT
    chmod +x "$bin_dir/dpkg" "$bin_dir/dpkg-deb"

    if PATH="$bin_dir:$PATH" APP_DIR_OVERRIDE="$workspace/does-not-exist" PKG_ROOT_OVERRIDE="$workspace/deb-root" "$REPO_DIR/scripts/build-deb.sh" >/dev/null 2>&1; then
        fail "build-deb.sh should fail when APP_DIR is missing"
    fi
}

test_make_build_app_uses_installer_download_flow_by_default() {
    info "Checking make build-app default DMG behavior"
    local workspace="$TMP_DIR/make-build-app"
    local install_log="$workspace/install-args.log"

    mkdir -p "$workspace"

    cat > "$workspace/install.sh" <<'SCRIPT'
#!/bin/bash
set -eu
printf '%s\n' "$#" > "$TEST_INSTALL_LOG"
if [ "$#" -gt 0 ]; then
    printf '%s\n' "$1" >> "$TEST_INSTALL_LOG"
fi
SCRIPT
    chmod +x "$workspace/install.sh"

    TEST_INSTALL_LOG="$install_log" make -f "$REPO_DIR/Makefile" -C "$workspace" build-app >/dev/null

    assert_file_exists "$install_log"
    first_line="$(sed -n '1p' "$install_log")"
    second_line="$(sed -n '2p' "$install_log")"
    [ "$first_line" = "1" ] || fail "Expected make build-app to call install.sh with a single default argument slot, got: $(cat "$install_log")"
    [ -z "$second_line" ] || fail "Expected make build-app default DMG argument to be empty so install.sh falls back to reuse/download, got: $(cat "$install_log")"
}

test_installer_writes_app_version_metadata() {
    info "Checking installer app version metadata extraction"
    local workspace="$TMP_DIR/installer-version"
    local app_bundle="$workspace/Codex.app"
    local install_dir="$workspace/codex-app"

    mkdir -p "$app_bundle/Contents"
    python3 - "$app_bundle/Contents/Info.plist" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "wb") as handle:
    plistlib.dump(
        {
            "CFBundleShortVersionString": "26.422.30944",
            "CFBundleVersion": "2080",
        },
        handle,
    )
PY

    (
        CODEX_INSTALLER_SKIP_MAIN=1
        CODEX_INSTALL_DIR="$install_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        write_app_version_metadata "$app_bundle"
    )

    assert_file_exists "$install_dir/codex-app-version.env"
    assert_contains "$install_dir/codex-app-version.env" "CODEX_APP_UPSTREAM_VERSION=26.422.30944"
    assert_contains "$install_dir/codex-app-version.env" "CODEX_APP_UPSTREAM_BUILD=2080"
    assert_contains "$install_dir/codex-app-version.env" "CODEX_APP_PACKAGE_VERSION=26.422.30944.2080"
}

test_installer_rejects_alphanumeric_app_version_metadata() {
    info "Checking installer app version metadata validation"
    local workspace="$TMP_DIR/installer-alpha-version"
    local app_bundle="$workspace/Codex.app"
    local install_dir="$workspace/codex-app"

    mkdir -p "$app_bundle/Contents"
    python3 - "$app_bundle/Contents/Info.plist" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "wb") as handle:
    plistlib.dump(
        {
            "CFBundleShortVersionString": "26.422.30944b",
            "CFBundleVersion": "2080",
        },
        handle,
    )
PY

    if (
        CODEX_INSTALLER_SKIP_MAIN=1
        CODEX_INSTALL_DIR="$install_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        write_app_version_metadata "$app_bundle"
    ) >/dev/null 2>&1; then
        fail "Expected installer to reject alphanumeric app package versions"
    fi
}

test_installer_rejects_short_app_version_metadata() {
    info "Checking installer app version segment validation"
    local workspace="$TMP_DIR/installer-short-version"
    local app_bundle="$workspace/Codex.app"
    local install_dir="$workspace/codex-app"

    mkdir -p "$app_bundle/Contents"
    python3 - "$app_bundle/Contents/Info.plist" <<'PY'
import plistlib
import sys

with open(sys.argv[1], "wb") as handle:
    plistlib.dump(
        {
            "CFBundleShortVersionString": "26",
            "CFBundleVersion": "2080",
        },
        handle,
    )
PY

    if (
        CODEX_INSTALLER_SKIP_MAIN=1
        CODEX_INSTALL_DIR="$install_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        write_app_version_metadata "$app_bundle"
    ) >/dev/null 2>&1; then
        fail "Expected installer to reject short app package versions"
    fi
}

test_launcher_template_sanity() {
    info "Checking launcher template markers"
    local workspace="$TMP_DIR/generated-launcher"
    local generated="$workspace/codex-app/start.sh"

    mkdir -p "$workspace"
    (
        CODEX_INSTALLER_SKIP_MAIN=1
        CODEX_INSTALL_DIR="$workspace/codex-app"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        mkdir -p "$CODEX_INSTALL_DIR"
        create_start_script
    )

    assert_file_exists "$generated"
    assert_contains "$generated" "nohup python3 -m http.server 5175 --bind 127.0.0.1"
    assert_contains "$generated" "saw_http_server=1"
    assert_contains "$generated" "saw_webview_port=1"
    assert_not_contains "$generated" "pkill -f \"http.server 5175\""
    assert_contains "$generated" "cleanup_launcher"
    assert_contains "$generated" 'kill "$ELECTRON_PID"'
    assert_contains "$generated" "STARTED_WEBVIEW_PID"
    assert_contains "$generated" "WEBVIEW_STARTED_BY_LAUNCHER"
    assert_contains "$generated" "WEBVIEW_PID_FILE"
    assert_contains "$generated" "ELECTRON_PID"
    assert_contains "$generated" "pid_is_main_electron_process"
    assert_contains "$generated" '[[ "$cmdline" != *"--type="* ]]'
    assert_contains "$generated" "detect_warm_start"
    assert_contains "$generated" "wait_for_webview_server"
    assert_contains "$generated" "verify_webview_origin"
    assert_contains "$generated" "Webview origin verified."
    assert_contains "$generated" 'echo "$ELECTRON_PID" > "$APP_PID_FILE"'
    assert_contains "$generated" "run_packaged_runtime_launch_check"
    local pid_write_line launch_check_line
    pid_write_line="$(grep -n 'echo "$ELECTRON_PID" > "$APP_PID_FILE"' "$generated" | head -n 1 | cut -d: -f1)"
    launch_check_line="$(grep -n 'run_packaged_runtime_launch_check' "$generated" | tail -n 1 | cut -d: -f1)"
    [ "$pid_write_line" -lt "$launch_check_line" ] || fail "Expected launch update check after app pid write"
    assert_contains "$generated" "--app-id=codex-app"
    assert_contains "$generated" "--ozone-platform-hint=auto"
    assert_contains "$generated" "CODEX_APP_DISABLE_ELECTRON_SANDBOX"
    assert_contains "$generated" "electron_args=("
    assert_not_contains "$generated" "    --no-sandbox \\\\"
    assert_not_contains "$generated" "    --disable-gpu-sandbox \\\\"
    assert_not_contains "$generated" 'exec "$SCRIPT_DIR'
    assert_contains "$generated" "PACKAGED_RUNTIME_HELPER"
    assert_contains "$generated" "prompt_install_missing_cli"
    assert_contains "$generated" "Install it now? \\[Y/n\\]"
    assert_contains "$generated" "is_interactive_terminal"
    assert_contains "$generated" "--cli-path \"\$CODEX_CLI_PATH\""
    assert_contains "$REPO_DIR/packaging/linux/packaged-runtime.sh" "CHROME_DESKTOP"
    assert_not_contains "$REPO_DIR/packaging/linux/packaged-runtime.sh" "        PATH \\\\"
    assert_contains "$REPO_DIR/packaging/linux/codex-app.desktop" "BAMF_DESKTOP_FILE_HINT"
}

test_launcher_lifecycle_cleans_child_processes() {
    info "Checking generated launcher process lifecycle"
    local workspace="$TMP_DIR/generated-launcher-lifecycle"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local http_pid_file="$workspace/http.pid"
    local electron_pid_file="$workspace/electron.pid"
    local electron_args_file="$workspace/electron.args"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local app_pid_file="$xdg_state/codex-app/app.pid"
    local launcher_pid=""

    mkdir -p "$workspace" "$bin_dir" "$app_dir/content/webview" "$xdg_cache" "$xdg_state"
    printf '<!doctype html><title>Codex</title><div>startup-loader</div>\n' > "$app_dir/content/webview/index.html"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$bin_dir/python3" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
case "${1:-} ${2:-}" in
    "-m http.server")
        printf '%s\n' "$$" > "$TEST_HTTP_PID_FILE"
        sleep 60
        ;;
    "-c "*)
        exit 0
        ;;
    "- "*)
        cat >/dev/null
        [ -s "$TEST_HTTP_PID_FILE" ]
        ;;
    *)
        exit 2
        ;;
esac
SCRIPT
    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$$" > "$TEST_ELECTRON_PID_FILE"
printf '%s\n' "$@" > "$TEST_ELECTRON_ARGS_FILE"
sleep 1
SCRIPT
    chmod +x "$bin_dir/python3" "$app_dir/electron" "$generated"

    TEST_HTTP_PID_FILE="$http_pid_file" \
    TEST_ELECTRON_PID_FILE="$electron_pid_file" \
    TEST_ELECTRON_ARGS_FILE="$electron_args_file" \
    PATH="$bin_dir:$PATH" \
    XDG_CACHE_HOME="$xdg_cache" \
    XDG_STATE_HOME="$xdg_state" \
    CODEX_CLI_PATH=/bin/true \
    CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
    "$generated" >/dev/null 2>&1 &
    launcher_pid="$!"

    local attempts
    for ((attempts = 0; attempts < 100; attempts += 1)); do
        [ -s "$http_pid_file" ] && [ -s "$electron_pid_file" ] && [ -s "$app_pid_file" ] && break
        sleep 0.05
    done
    [ -s "$http_pid_file" ] || fail "Expected launcher to start webview server"
    [ -s "$electron_pid_file" ] || fail "Expected launcher to start Electron"
    [ -s "$app_pid_file" ] || fail "Expected launcher to write app pid"

    local http_pid electron_pid recorded_app_pid
    http_pid="$(cat "$http_pid_file")"
    electron_pid="$(cat "$electron_pid_file")"
    recorded_app_pid="$(cat "$app_pid_file")"
    [ "$recorded_app_pid" = "$electron_pid" ] || fail "Expected app pid to track Electron pid"
    assert_contains "$electron_args_file" "--no-sandbox"
    assert_contains "$electron_args_file" "--disable-gpu-sandbox"

    wait "$launcher_pid" || fail "Expected generated launcher to exit with Electron"
    launcher_pid=""
    sleep 0.1

    if kill -0 "$http_pid" 2>/dev/null; then
        fail "Expected launcher cleanup to stop webview server"
    fi
    if kill -0 "$electron_pid" 2>/dev/null; then
        fail "Expected Electron fixture to exit"
    fi
    [ ! -e "$app_pid_file" ] || fail "Expected launcher cleanup to remove app pid"
}

test_launcher_uses_updater_discovered_cli_without_path_codex() {
    info "Checking launcher accepts updater CLI discovery before direct PATH fallback"
    local workspace="$TMP_DIR/generated-launcher-updater-cli"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local updater_args_file="$workspace/updater.args"
    local discovered_cli="$workspace/fallback/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$bin_dir" "$app_dir" "$xdg_cache" "$xdg_state" "$(dirname "$discovered_cli")"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$bin_dir/codex-app-updater" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" > "$TEST_UPDATER_ARGS_FILE"
printf 'updater warning on stderr\n' >&2
printf '%s\n' "$TEST_DISCOVERED_CODEX"
SCRIPT
    cat > "$discovered_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    chmod +x "$app_dir/electron" "$bin_dir/codex-app-updater" "$discovered_cli" "$generated"

    set +e
    TEST_ELECTRON_ENV_FILE="$electron_env_file" \
        TEST_UPDATER_ARGS_FILE="$updater_args_file" \
        TEST_DISCOVERED_CODEX="$discovered_cli" \
        PATH="$bin_dir:/usr/bin" \
        XDG_CACHE_HOME="$xdg_cache" \
        XDG_STATE_HOME="$xdg_state" \
        CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
        "$generated" >/dev/null 2>&1
    local launcher_status=$?
    set -e
    if [ "$launcher_status" -ne 0 ]; then
        [ ! -f "$launcher_log" ] || sed 's/^/[launcher-log] /' "$launcher_log" >&2
        fail "Expected launcher to continue with updater-discovered Codex CLI path"
    fi

    [ "$(cat "$electron_env_file")" = "$discovered_cli" ] || fail "Expected updater-discovered Codex CLI path to reach Electron"
    assert_contains "$updater_args_file" "cli-preflight"
    assert_contains "$updater_args_file" "--print-path"
    assert_not_contains "$updater_args_file" "--cli-path"
    assert_contains "$launcher_log" "updater warning on stderr"
    assert_not_contains "$launcher_log" "Codex CLI not found"
}

test_launcher_falls_back_to_path_codex_when_updater_preflight_fails() {
    info "Checking launcher falls back to PATH codex after updater preflight failure"
    local workspace="$TMP_DIR/generated-launcher-updater-fallback"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local updater_args_file="$workspace/updater.args"
    local path_cli="$bin_dir/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$bin_dir" "$app_dir" "$xdg_cache" "$xdg_state"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$bin_dir/codex-app-updater" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" > "$TEST_UPDATER_ARGS_FILE"
exit 1
SCRIPT
    cat > "$path_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    chmod +x "$app_dir/electron" "$bin_dir/codex-app-updater" "$path_cli" "$generated"

    TEST_ELECTRON_ENV_FILE="$electron_env_file" \
    TEST_UPDATER_ARGS_FILE="$updater_args_file" \
    PATH="$bin_dir:/usr/bin" \
    XDG_CACHE_HOME="$xdg_cache" \
    XDG_STATE_HOME="$xdg_state" \
    CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
    "$generated" >/dev/null 2>&1

    [ "$(cat "$electron_env_file")" = "$path_cli" ] || fail "Expected PATH Codex CLI fallback to reach Electron"
    assert_contains "$updater_args_file" "cli-preflight"
    assert_contains "$updater_args_file" "--print-path"
    assert_contains "$launcher_log" "Codex CLI prelaunch check failed"
    assert_not_contains "$launcher_log" "unbound variable"
    assert_not_contains "$launcher_log" "Codex CLI not found"
}

test_launcher_skips_relative_path_codex_and_uses_later_absolute_path() {
    info "Checking launcher skips relative PATH Codex CLI entries"
    local workspace="$TMP_DIR/generated-launcher-relative-path-skip"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local rel_bin="$workspace/relbin"
    local tool_bin="$workspace/bin"
    local home_dir="$workspace/home"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local path_cli="$tool_bin/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$rel_bin" "$tool_bin" "$home_dir" "$app_dir" "$xdg_cache" "$xdg_state"
    local tool
    for tool in date dirname id mkdir tee uname; do
        ln -s "/usr/bin/$tool" "$tool_bin/$tool"
    done
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$rel_bin/codex" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.13.0\n'
SCRIPT
    cat > "$path_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    chmod +x "$app_dir/electron" "$rel_bin/codex" "$path_cli" "$generated"

    (
        cd "$workspace"
        TEST_ELECTRON_ENV_FILE="$electron_env_file" \
        HOME="$home_dir" \
        PNPM_HOME="" \
        BUN_INSTALL="" \
        VOLTA_HOME="" \
        ASDF_DATA_DIR="" \
        MISE_DATA_DIR="" \
        PATH="relbin:$tool_bin" \
        XDG_CACHE_HOME="$xdg_cache" \
        XDG_STATE_HOME="$xdg_state" \
        CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
        "$generated" >/dev/null 2>&1
    )

    [ "$(cat "$electron_env_file")" = "$path_cli" ] || fail "Expected launcher to skip relative PATH Codex CLI entry"
    assert_not_contains "$launcher_log" "Codex CLI not found"
}

test_launcher_falls_back_to_known_cli_without_updater() {
    info "Checking launcher fallback finds known Codex CLI paths without updater"
    local workspace="$TMP_DIR/generated-launcher-known-cli-fallback"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local home_dir="$workspace/home"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local known_cli="$home_dir/.local/bin/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$bin_dir" "$app_dir" "$xdg_cache" "$xdg_state" "$(dirname "$known_cli")"
    local tool
    for tool in date dirname id mkdir tee uname; do
        ln -s "/usr/bin/$tool" "$bin_dir/$tool"
    done
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$known_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    chmod +x "$app_dir/electron" "$known_cli" "$generated"

    TEST_ELECTRON_ENV_FILE="$electron_env_file" \
    HOME="$home_dir" \
    PNPM_HOME="" \
    BUN_INSTALL="" \
    VOLTA_HOME="" \
    ASDF_DATA_DIR="" \
    MISE_DATA_DIR="" \
    PATH="$bin_dir" \
    XDG_CACHE_HOME="$xdg_cache" \
    XDG_STATE_HOME="$xdg_state" \
    CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
    "$generated" >/dev/null 2>&1

    [ "$(cat "$electron_env_file")" = "$known_cli" ] || fail "Expected known fallback Codex CLI path to reach Electron"
    assert_not_contains "$launcher_log" "Codex CLI not found"
}

test_launcher_retains_updater_printed_cli_after_preflight_failure() {
    info "Checking launcher keeps updater-discovered CLI after non-fatal preflight failure"
    local workspace="$TMP_DIR/generated-launcher-updater-failed-with-path"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local updater_args_file="$workspace/updater.args"
    local discovered_cli="$workspace/configured/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$bin_dir" "$app_dir" "$xdg_cache" "$xdg_state" "$(dirname "$discovered_cli")"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$bin_dir/codex-app-updater" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" > "$TEST_UPDATER_ARGS_FILE"
printf 'registry lookup failed\n' >&2
printf '%s\n' "$TEST_DISCOVERED_CODEX"
exit 1
SCRIPT
    cat > "$discovered_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    chmod +x "$app_dir/electron" "$bin_dir/codex-app-updater" "$discovered_cli" "$generated"

    set +e
    TEST_ELECTRON_ENV_FILE="$electron_env_file" \
        TEST_UPDATER_ARGS_FILE="$updater_args_file" \
        TEST_DISCOVERED_CODEX="$discovered_cli" \
        PATH="$bin_dir:/usr/bin" \
        XDG_CACHE_HOME="$xdg_cache" \
        XDG_STATE_HOME="$xdg_state" \
        CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
        "$generated" >/dev/null 2>&1
    local launcher_status=$?
    set -e
    if [ "$launcher_status" -ne 0 ]; then
        [ ! -f "$launcher_log" ] || sed 's/^/[launcher-log] /' "$launcher_log" >&2
        fail "Expected launcher to continue with updater-printed Codex CLI path after preflight failure"
    fi

    [ "$(cat "$electron_env_file")" = "$discovered_cli" ] || fail "Expected updater-printed Codex CLI path to reach Electron after preflight failure"
    assert_contains "$updater_args_file" "cli-preflight"
    assert_contains "$updater_args_file" "--print-path"
    assert_contains "$launcher_log" "Codex CLI prelaunch check failed"
    assert_not_contains "$launcher_log" "Codex CLI not found"
}

test_launcher_rejects_invalid_configured_cli_before_path_fallback() {
    info "Checking launcher rejects invalid configured CLI before PATH fallback"
    local workspace="$TMP_DIR/generated-launcher-invalid-configured-cli"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local updater_args_file="$workspace/updater.args"
    local path_cli="$bin_dir/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$bin_dir" "$app_dir" "$xdg_cache" "$xdg_state"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$bin_dir/codex-app-updater" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" > "$TEST_UPDATER_ARGS_FILE"
printf 'Invalid config Codex CLI path /tmp/not-codex: expected an executable regular file\n' >&2
exit 78
SCRIPT
    cat > "$path_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    chmod +x "$app_dir/electron" "$bin_dir/codex-app-updater" "$path_cli" "$generated"

    set +e
    TEST_ELECTRON_ENV_FILE="$electron_env_file" \
        TEST_UPDATER_ARGS_FILE="$updater_args_file" \
        PATH="$bin_dir:/usr/bin" \
        XDG_CACHE_HOME="$xdg_cache" \
        XDG_STATE_HOME="$xdg_state" \
        CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
        "$generated" >/dev/null 2>&1
    local launcher_status=$?
    set -e
    [ "$launcher_status" -eq 78 ] || fail "Expected invalid configured Codex CLI path to exit 78, got $launcher_status"

    [ ! -e "$electron_env_file" ] || fail "Expected Electron not to launch after invalid configured CLI path"
    assert_contains "$updater_args_file" "cli-preflight"
    assert_contains "$updater_args_file" "--print-path"
    assert_contains "$launcher_log" "Invalid config Codex CLI path"
    assert_not_contains "$launcher_log" "Using CODEX_CLI_PATH=$path_cli"
}

test_launcher_rejects_invalid_env_cli_with_updater_before_path_fallback() {
    info "Checking launcher rejects invalid env CLI with updater before PATH fallback"
    local workspace="$TMP_DIR/generated-launcher-invalid-env-cli"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local bin_dir="$workspace/bin"
    local xdg_cache="$workspace/cache"
    local xdg_state="$workspace/state"
    local electron_env_file="$workspace/electron.env"
    local path_cli="$bin_dir/codex"
    local invalid_cli="$workspace/missing/codex"
    local launcher_log="$xdg_cache/codex-app/launcher.log"

    mkdir -p "$workspace" "$bin_dir" "$app_dir" "$xdg_cache" "$xdg_state"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    cat > "$app_dir/electron" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$CODEX_CLI_PATH" > "$TEST_ELECTRON_ENV_FILE"
SCRIPT
    cat > "$path_cli" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
printf 'codex-cli v0.42.0\n'
SCRIPT
    cat > "$bin_dir/codex-app-updater" <<'SCRIPT'
#!/bin/bash
set -euo pipefail
exit 1
SCRIPT
    chmod +x "$app_dir/electron" "$bin_dir/codex-app-updater" "$path_cli" "$generated"

    set +e
    TEST_ELECTRON_ENV_FILE="$electron_env_file" \
        PATH="$bin_dir:/usr/bin" \
        XDG_CACHE_HOME="$xdg_cache" \
        XDG_STATE_HOME="$xdg_state" \
        CODEX_CLI_PATH="$invalid_cli" \
        CODEX_APP_DISABLE_ELECTRON_SANDBOX=1 \
        "$generated" >/dev/null 2>&1
    local launcher_status=$?
    set -e
    [ "$launcher_status" -eq 78 ] || fail "Expected invalid CODEX_CLI_PATH to exit 78, got $launcher_status"

    [ ! -e "$electron_env_file" ] || fail "Expected Electron not to launch after invalid CODEX_CLI_PATH"
    assert_contains "$launcher_log" "Invalid environment Codex CLI path $invalid_cli"
    assert_not_contains "$launcher_log" "Using CODEX_CLI_PATH=$path_cli"
}

test_launcher_help_preserves_existing_app_pid() {
    info "Checking generated launcher preserves unrelated app pid on early exit"
    local workspace="$TMP_DIR/generated-launcher-help-pid"
    local app_dir="$workspace/codex-app"
    local generated="$app_dir/start.sh"
    local cache_dir="$workspace/cache"
    local state_dir="$workspace/state"
    local app_pid_file="$state_dir/codex-app/app.pid"

    mkdir -p "$workspace" "$app_dir" "$(dirname "$app_pid_file")"
    (
        export CODEX_INSTALLER_SKIP_MAIN=1
        export CODEX_INSTALL_DIR="$app_dir"
        # shellcheck disable=SC1091
        source "$REPO_DIR/install.sh"
        create_start_script
    )

    printf '123456\n' > "$app_pid_file"
    XDG_CACHE_HOME="$cache_dir" XDG_STATE_HOME="$state_dir" "$generated" --help >/dev/null
    [ "$(cat "$app_pid_file")" = "123456" ] || fail "Expected help path to preserve unrelated app pid"
}

test_hash_workflow_opens_review_pr() {
    info "Checking hash refresh workflow requires review"
    local workflow="$REPO_DIR/.github/workflows/update-codex-hash.yml"

    assert_contains "$workflow" "pull-requests: write"
    assert_contains "$workflow" "GH_TOKEN: \${{ github.token }}"
    assert_contains "$workflow" "gh pr list --base main --head \"\$BRANCH\" --state open"
    assert_contains "$workflow" "gh pr edit \"\$PR_NUMBER\""
    assert_contains "$workflow" "gh pr create"
    assert_contains "$workflow" "git fetch origin \"\$BRANCH:refs/remotes/origin/\$BRANCH\" || true"
    assert_not_contains "$workflow" "git push origin main"
    assert_not_contains "$workflow" "gh pr view \"\$BRANCH\" --base main"
    assert_not_contains "$workflow" "grep -oP"
    assert_not_contains "$workflow" "actions/checkout@v4"
    assert_not_contains "$workflow" "cachix/install-nix-action@v27"
}

test_apple_dmg_verifier_pins_upstream_trust_inputs() {
    info "Checking Apple DMG verifier pins upstream trust inputs"
    local script="$REPO_DIR/scripts/verify-apple-dmg.sh"

    assert_contains "$script" "EXPECTED_BUNDLE_ID=\"com.openai.codex\""
    assert_contains "$script" "EXPECTED_TEAM_ID=\"2DC432GLL2\""
    assert_contains "$script" "Developer ID Application: OpenAI OpCo, LLC (2DC432GLL2)"
    assert_contains "$script" "EXPECTED_SPARKLE_KEY=\"rhcBvttuqDFriyNqwTQJR3L4UT1WjIK4QxtwtwusVic=\""
    assert_contains "$script" "Set CODEX_DMG_SHA256 or CODEX_DMG_SRI before Apple DMG verification"
    assert_contains "$script" "codesign --verify --deep --strict --verbose=4"
    assert_contains "$script" "spctl -a -vvv -t exec"
    assert_contains "$script" "spctl -a -t open --context context:primary-signature"
    assert_contains "$script" "xcrun stapler validate"
    assert_contains "$script" "hdiutil verify"
    assert_contains "$script" "flake_dmg_sri"
    assert_contains "$script" "Codex\\.dmg"
    assert_contains "$script" "find \"\$MOUNT_DIR\" -maxdepth 4 -type d -name 'Codex\\.app'"
    assert_contains "$script" "Expected exactly one Codex\\.app under"
    assert_contains "$script" "CODEX_REQUIRE_DMG_GATEKEEPER"
    assert_contains "$script" "CODEX_REQUIRE_DMG_STAPLE"
    assert_not_contains "$script" "Codex Installer/Codex.app"
    assert_not_contains "$script" "CODEX_EXPECTED_BUNDLE_ID"
    assert_not_contains "$script" "CODEX_EXPECTED_APPLE_TEAM_ID"
    assert_not_contains "$script" "CODEX_EXPECTED_DEVELOPER_ID"
    assert_not_contains "$script" "CODEX_EXPECTED_SPARKLE_PUBLIC_ED_KEY"
    assert_not_contains "$script" "--app"
}

test_apple_dmg_workflow_runs_on_macos() {
    info "Checking Apple DMG verification workflow"
    local workflow="$REPO_DIR/.github/workflows/verify-apple-dmg.yml"

    assert_contains "$workflow" "workflow_dispatch:"
    assert_contains "$workflow" "runs-on: macos-15"
    assert_contains "$workflow" "actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5"
    assert_not_contains "$workflow" "actions/checkout@v"
    assert_contains "$workflow" "curl -fL --retry 3 --output Codex.dmg"
    assert_contains "$workflow" "./scripts/verify-apple-dmg.sh --dmg Codex.dmg"
    assert_contains "$workflow" "CODEX_DMG_SHA256: \${{ inputs.dmg_sha256 }}"
    assert_contains "$workflow" "CODEX_REQUIRE_DMG_GATEKEEPER"
    assert_contains "$workflow" "CODEX_REQUIRE_DMG_STAPLE"
}

test_updater_service_hardening() {
    info "Checking updater service hardening"
    local service="$REPO_DIR/packaging/linux/codex-app-updater.service"

    assert_not_contains "$service" "NoNewPrivileges=yes"
    assert_contains "$service" "PrivateTmp=yes"
    assert_contains "$service" "RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6"
    assert_contains "$service" "Environment=PATH=/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:%h/.local/bin"
    assert_contains "$service" "UMask=077"
}

test_packaged_runtime_preserves_active_updater_service() {
    info "Checking packaged runtime does not restart active updater service"
    local runtime="$REPO_DIR/packaging/linux/packaged-runtime.sh"

    assert_contains "$runtime" "codex_packaged_runtime_launch_check"
    assert_contains "$runtime" "systemctl --user is-active codex-app-updater.service"
    assert_contains "$runtime" "systemctl --user start codex-app-updater.service"
    assert_not_contains "$runtime" "systemctl --user restart codex-app-updater.service"
}

test_electron_security_inspector_flags_insecure_generated_app() {
    info "Checking Electron security inspector flags insecure generated app settings"
    local workspace="$TMP_DIR/electron-security"
    local report="$workspace/report.txt"
    mkdir -p "$workspace/app"
    cat > "$workspace/app/main.js" <<'SCRIPT'
const { BrowserWindow, shell } = require('electron')
new BrowserWindow({
  webPreferences: {
    nodeIntegration: true,
    contextIsolation: false,
    sandbox: false
  }
})
shell.openExternal(url)
new BrowserWindow({ webPreferences: { webSecurity: false } })
SCRIPT
    cat > "$workspace/app/index.html" <<'HTML'
<webview src="https://example.invalid" nodeintegration allowpopups></webview>
HTML

    if node "$REPO_DIR/scripts/inspect-electron-security.js" "$workspace/app" > "$report" 2>&1; then
        fail "Expected Electron security inspector to reject insecure fixture"
    fi

    assert_contains "$report" "nodeIntegration: true"
    assert_contains "$report" "contextIsolation: false"
    assert_contains "$report" "sandbox: false"
    assert_contains "$report" "webSecurity: false"
    assert_contains "$report" "<webview> enables Node.js integration"
    assert_contains "$report" "shell.openExternal"
}

test_release_gate_requires_verified_dmg_hash() {
    info "Checking release gate requires verified DMG hash"
    local workspace="$TMP_DIR/release-gate-missing-hash"

    mkdir -p "$workspace/codex-app" "$workspace/dist"
    printf 'dmg\n' > "$workspace/Codex.dmg"
    printf 'console.log("safe")\n' > "$workspace/codex-app/main.js"
    printf 'package\n' > "$workspace/dist/codex-app_26.422.30944.2080_amd64.deb"

    if (
        cd "$workspace"
        PATH="$REPO_DIR/scripts:$PATH" \
        CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA=1 \
        "$REPO_DIR/scripts/release-gate.sh" >/dev/null 2>&1
    ); then
        fail "Expected release gate to reject missing trusted DMG hash"
    fi
}

test_release_gate_does_not_fetch_asar_with_npx() {
    info "Checking release gate does not fetch asar with npx"
    assert_not_contains "$REPO_DIR/scripts/release-gate.sh" "npx --yes asar"
    assert_contains "$REPO_DIR/scripts/release-gate.sh" "npx --no-install asar"
    assert_not_contains "$REPO_DIR/scripts/release-gate.sh" "grep -oP"
    assert_contains "$REPO_DIR/scripts/release-gate.sh" "flake_dmg_sri"
}

test_release_gate_writes_checksums_and_requires_signature() {
    info "Checking release gate writes checksums and required signature"
    local workspace="$TMP_DIR/release-gate-checksums"
    local bin_dir="$workspace/bin"
    local expected_hash

    mkdir -p "$workspace/codex-app/resources" "$workspace/asar-extracted" "$workspace/dist" "$bin_dir"
    printf 'dmg\n' > "$workspace/Codex.dmg"
    expected_hash="$(sha256sum "$workspace/Codex.dmg" | awk '{print $1}')"
    printf 'asar\n' > "$workspace/codex-app/resources/app.asar"
    printf 'console.log("safe")\n' > "$workspace/asar-extracted/main.js"
    printf 'package\n' > "$workspace/dist/codex-app_26.422.30944.2080_amd64.deb"
    printf 'signature\n' > "$workspace/dist/codex-app-26.422.30944.2080-1-x86_64.pkg.tar.zst.sig"
    cat > "$bin_dir/gpg" <<'SCRIPT'
#!/bin/bash
output=""
mode=""
args=("$@")
for arg in "${args[@]}"; do
    case "$arg" in
        --export) mode="export" ;;
        --import) mode="import" ;;
        --verify) mode="verify" ;;
    esac
done
case "$mode" in
    export)
        printf '%s\n' '-----BEGIN PGP PUBLIC KEY BLOCK-----'
        printf '%s\n' 'test'
        printf '%s\n' '-----END PGP PUBLIC KEY BLOCK-----'
        exit 0
        ;;
    import|verify)
        exit 0
        ;;
esac
while [ "$#" -gt 0 ]; do
    if [ "$1" = "--output" ]; then
        output="$2"
        shift 2
        continue
    fi
    shift
done
[ -n "$output" ] || exit 2
printf 'signature\n' > "$output"
SCRIPT
    cat > "$bin_dir/asar" <<'SCRIPT'
#!/bin/bash
if [ "$1" != "extract" ]; then
    exit 2
fi
mkdir -p "$3"
cp -a "$TEST_ASAR_EXTRACT_SOURCE/." "$3/"
SCRIPT
    chmod +x "$bin_dir/gpg"
    chmod +x "$bin_dir/asar"

    (
        cd "$workspace"
        PATH="$bin_dir:$PATH" \
        CODEX_DMG_SHA256="$expected_hash" \
        CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA=1 \
        REQUIRE_RELEASE_SIGNATURE=1 \
        CODEX_RELEASE_GPG_KEY="test@example.invalid" \
        TEST_ASAR_EXTRACT_SOURCE="$workspace/asar-extracted" \
        "$REPO_DIR/scripts/release-gate.sh"
    )

    assert_file_exists "$workspace/dist/SHA256SUMS"
    assert_file_exists "$workspace/dist/SHA256SUMS.asc"
    assert_file_exists "$workspace/dist/release-signing-key.asc"
    assert_contains "$workspace/dist/SHA256SUMS" "codex-app_26.422.30944.2080_amd64.deb"
    assert_contains "$workspace/dist/release-signing-key.asc" "BEGIN PGP PUBLIC KEY"
    assert_not_contains "$workspace/dist/SHA256SUMS" ".pkg.tar.zst.sig"
}

test_release_gate_removes_stale_signature_when_unsigned() {
    info "Checking release gate removes stale checksum signature for unsigned runs"
    local workspace="$TMP_DIR/release-gate-stale-signature"
    local bin_dir="$workspace/bin"
    local expected_hash

    mkdir -p "$workspace/codex-app/resources" "$workspace/asar-extracted" "$workspace/dist" "$bin_dir"
    printf 'dmg\n' > "$workspace/Codex.dmg"
    expected_hash="$(sha256sum "$workspace/Codex.dmg" | awk '{print $1}')"
    printf 'asar\n' > "$workspace/codex-app/resources/app.asar"
    printf 'console.log("safe")\n' > "$workspace/asar-extracted/main.js"
    printf 'package\n' > "$workspace/dist/codex-app_26.422.30944.2080_amd64.deb"
    printf 'stale-signature\n' > "$workspace/dist/SHA256SUMS.asc"
    cat > "$bin_dir/asar" <<'SCRIPT'
#!/bin/bash
if [ "$1" != "extract" ]; then
    exit 2
fi
mkdir -p "$3"
cp -a "$TEST_ASAR_EXTRACT_SOURCE/." "$3/"
SCRIPT
    chmod +x "$bin_dir/asar"

    (
        cd "$workspace"
        PATH="$bin_dir:$PATH" \
        CODEX_DMG_SHA256="$expected_hash" \
        CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA=1 \
        TEST_ASAR_EXTRACT_SOURCE="$workspace/asar-extracted" \
        "$REPO_DIR/scripts/release-gate.sh"
    )

    [ ! -e "$workspace/dist/SHA256SUMS.asc" ] || fail "Expected unsigned release gate to remove stale signature"
}

test_release_gate_fails_on_insecure_asar_contents() {
    info "Checking release gate scans extracted app.asar contents"
    local workspace="$TMP_DIR/release-gate-insecure-asar"
    local bin_dir="$workspace/bin"
    local expected_hash

    mkdir -p "$workspace/codex-app/resources" "$workspace/asar-extracted" "$workspace/dist" "$bin_dir"
    printf 'dmg\n' > "$workspace/Codex.dmg"
    expected_hash="$(sha256sum "$workspace/Codex.dmg" | awk '{print $1}')"
    printf 'asar\n' > "$workspace/codex-app/resources/app.asar"
    printf 'new BrowserWindow({webPreferences:{nodeIntegration:true}})\n' > "$workspace/asar-extracted/main.js"
    printf 'package\n' > "$workspace/dist/codex-app_26.422.30944.2080_amd64.deb"
    cat > "$bin_dir/asar" <<'SCRIPT'
#!/bin/bash
if [ "$1" != "extract" ]; then
    exit 2
fi
mkdir -p "$3"
cp -a "$TEST_ASAR_EXTRACT_SOURCE/." "$3/"
SCRIPT
    chmod +x "$bin_dir/asar"

    if (
        cd "$workspace"
        PATH="$bin_dir:$PATH" \
        CODEX_DMG_SHA256="$expected_hash" \
        CODEX_RELEASE_GATE_SKIP_PACKAGE_METADATA=1 \
        TEST_ASAR_EXTRACT_SOURCE="$workspace/asar-extracted" \
        "$REPO_DIR/scripts/release-gate.sh" >/dev/null 2>&1
    ); then
        fail "Expected release gate to fail on insecure app.asar contents"
    fi
}

make_fake_extracted_asar() {
    local root="$1"
    local bundle_body="$2"
    local settings_body="${3:-}"
    local index_body="${4:-}"

    mkdir -p "$root/webview/assets" "$root/.vite/build"
    printf 'png' > "$root/webview/assets/app-test.png"
    if [ -n "$settings_body" ]; then
        printf '%s\n' "$settings_body" > "$root/webview/assets/code-theme-test.js"
    fi
    if [ -n "$index_body" ]; then
        printf '%s\n' "$index_body" > "$root/webview/assets/use-resolved-theme-variant-test.js"
    fi
    cat > "$root/package.json" <<'JSON'
{}
JSON
    printf '%s\n' "$bundle_body" > "$root/.vite/build/main-test.js"
}

test_linux_file_manager_patch_smoke() {
    info "Checking Linux file manager patch behavior"
    local workspace="$TMP_DIR/file-manager-patch"
    local extracted="$workspace/extracted"
    local output_log="$workspace/output.log"

    mkdir -p "$workspace"
    make_fake_extracted_asar "$extracted" 'let D={removeMenu(){},setMenuBarVisibility(){},setIcon(){},once(){}};let t={join(){}};let a={existsSync(){return true},statSync(){return {isFile(){return false}}}};let n={shell:{openPath(){return ""},showItemInFolder(){}}};...process.platform===`win32`?{autoHideMenuBar:!0}:{},process.platform===`win32`&&D.removeMenu(),foo)}),D.once(`ready-to-show`,()=>{var sa=Mi({id:`fileManager`,label:`Finder`,icon:`apps/finder.png`,kind:`fileManager`,darwin:{detect:()=>`open`,args:e=>ai(e)},win32:{label:`File Explorer`,icon:`apps/file-explorer.png`,detect:ca,args:e=>ai(e),open:async({path:e})=>la(e)}});function ca(){let e=1;return e}async function la(e){let t=ua(e);if(t&&(0,a.statSync)(t).isFile()){n.shell.showItemInFolder(t);return}let r=t??e,i=await n.shell.openPath(r);if(i)throw Error(i)}function ua(e){return e}var Ua=Mi({id:`systemDefault`,label:`System Default App`,icon:`apps/file-explorer.png`,kind:`systemDefault`,hidden:!0,darwin:{icon:`apps/finder.png`,detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)},win32:{detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)},linux:{detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)}});async function Wa(e){return e}'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_contains "$extracted/.vite/build/main-test.js" 'detect:()=>`linux-file-manager`'
    assert_contains "$extracted/.vite/build/main-test.js" 'linux:{label:`File Manager`'
    assert_contains "$extracted/.vite/build/main-test.js" 'process.platform===`linux`&&D.setMenuBarVisibility(!1),'
    assert_contains "$extracted/.vite/build/main-test.js" '&&D.setIcon('
    assert_not_contains "$output_log" 'Failed to apply Linux File Manager Patch'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_not_contains "$output_log" 'Failed to apply Linux File Manager Patch'
}

test_linux_translucent_sidebar_default_patch_smoke() {
    info "Checking Linux translucent sidebar default patch behavior"
    local workspace="$TMP_DIR/translucent-sidebar-patch"
    local extracted="$workspace/extracted"
    local output_log="$workspace/output.log"

    mkdir -p "$workspace"
    make_fake_extracted_asar \
        "$extracted" \
        'let D={removeMenu(){},setMenuBarVisibility(){},setIcon(){},once(){}};let t={join(){}};let a={existsSync(){return true},statSync(){return {isFile(){return false}}}};let n={shell:{openPath(){return ""},showItemInFolder(){}}};...process.platform===`win32`?{autoHideMenuBar:!0}:{},process.platform===`win32`&&D.removeMenu(),foo)}),D.once(`ready-to-show`,()=>{var sa=Mi({id:`fileManager`,label:`Finder`,icon:`apps/finder.png`,kind:`fileManager`,darwin:{detect:()=>`open`,args:e=>ai(e)},win32:{label:`File Explorer`,icon:`apps/file-explorer.png`,detect:ca,args:e=>ai(e),open:async({path:e})=>la(e)}});function ca(){let e=1;return e}async function la(e){let t=ua(e);if(t&&(0,a.statSync)(t).isFile()){n.shell.showItemInFolder(t);return}let r=t??e,i=await n.shell.openPath(r);if(i)throw Error(i)}function ua(e){return e}var Ua=Mi({id:`systemDefault`,label:`System Default App`,icon:`apps/file-explorer.png`,kind:`systemDefault`,hidden:!0,darwin:{icon:`apps/finder.png`,detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)},win32:{detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)},linux:{detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)}});async function Wa(e){return e}' \
        'function settings(){return {opaqueWindows:e?.opaqueWindows??n.opaqueWindows,semanticColors:{}}}' \
        'function runtime(){return {opaqueWindows:e?.opaqueWindows??n.opaqueWindows,semanticColors:{}}}'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_contains "$extracted/webview/assets/code-theme-test.js" 'opaqueWindows:e?.opaqueWindows??(typeof navigator<`u`&&((navigator.userAgentData?.platform??navigator.platform??navigator.userAgent).toLowerCase().includes(`linux`))?!0:n.opaqueWindows),semanticColors:'
    assert_contains "$extracted/webview/assets/use-resolved-theme-variant-test.js" 'opaqueWindows:e?.opaqueWindows??(typeof navigator<`u`&&((navigator.userAgentData?.platform??navigator.platform??navigator.userAgent).toLowerCase().includes(`linux`))?!0:n.opaqueWindows),semanticColors:'
    assert_occurrence_count "$extracted/webview/assets/code-theme-test.js" 'toLowerCase().includes(`linux`)' '1'
    assert_occurrence_count "$extracted/webview/assets/use-resolved-theme-variant-test.js" 'toLowerCase().includes(`linux`)' '1'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_occurrence_count "$extracted/webview/assets/code-theme-test.js" 'toLowerCase().includes(`linux`)' '1'
    assert_occurrence_count "$extracted/webview/assets/use-resolved-theme-variant-test.js" 'toLowerCase().includes(`linux`)' '1'
}

test_linux_tray_startup_patch_smoke() {
    info "Checking Linux tray startup patch behavior"
    local workspace="$TMP_DIR/tray-startup-patch"
    local extracted="$workspace/extracted"
    local output_log="$workspace/output.log"

    mkdir -p "$workspace"
    make_fake_extracted_asar "$extracted" 'process.platform!==`win32`&&process.platform!==`darwin`?null:new n.Tray(icon);async function icons(){for(let e of o){let t=n.nativeImage.createFromPath(e);if(!t.isEmpty())return{defaultIcon:t,chronicleRunningIcon:null}}return{defaultIcon:await n.app.getFileIcon(process.execPath,{size:process.platform===`win32`?`small`:`normal`}),chronicleRunningIcon:null}}if(process.platform===`win32`&&f===`local`&&!this.isAppQuitting&&this.options.canHideLastLocalWindowToTray?.()===!0&&!t){e.preventDefault(),k.hide();return}class TrayController{trayMenuThreads={runningThreads:[],unreadThreads:[],pinnedThreads:[],recentThreads:[],usageLimits:[]};constructor(){}init(){this.tray.on(`click`,()=>{this.onTrayButtonClick()}),this.tray.on(`right-click`,()=>{this.openNativeTrayMenu()})}openNativeTrayMenu(){this.updateChronicleTrayIcon();let e=n.Menu.buildFromTemplate(this.getNativeTrayMenuItems());e.once(`menu-will-show`,()=>{this.isNativeTrayMenuOpen=!0}),e.once(`menu-will-close`,()=>{this.isNativeTrayMenuOpen=!1,this.handleNativeTrayMenuClosed()}),this.tray.popUpContextMenu(e)}update(e){switch(e.type){case`tray-menu-threads-changed`:this.trayMenuThreads=e.trayMenuThreads;return}E&&oe();}}'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_not_contains "$output_log" 'skipping Linux tray patch'
    assert_contains "$extracted/.vite/build/main-test.js" '(E||process.platform===`linux`)&&oe();'
    assert_contains "$extracted/.vite/build/main-test.js" 'this.trayMenuThreads=e.trayMenuThreads,process.platform===`linux`&&this.setLinuxTrayContextMenu?.();return'
    assert_occurrence_count "$extracted/.vite/build/main-test.js" '(E||process.platform===`linux`)&&oe();' '1'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_not_contains "$output_log" 'skipping Linux tray patch'
    assert_occurrence_count "$extracted/.vite/build/main-test.js" '(E||process.platform===`linux`)&&oe();' '1'
}

test_linux_file_manager_patch_fails_soft() {
    info "Checking Linux file manager patch fallback"
    local workspace="$TMP_DIR/file-manager-patch-fallback"
    local extracted="$workspace/extracted"
    local output_log="$workspace/output.log"

    mkdir -p "$workspace"
    make_fake_extracted_asar "$extracted" 'let D={removeMenu(){},setMenuBarVisibility(){},setIcon(){},once(){}};let t={join(){}};...process.platform===`win32`?{autoHideMenuBar:!0}:{},process.platform===`win32`&&D.removeMenu(),foo)}),D.once(`ready-to-show`,()=>{var brokenFileManager=Mi({id:`fileManager`,label:`Finder`,icon:`apps/finder.png`,kind:`fileManager`});var Ua=Mi({id:`systemDefault`,label:`System Default App`,icon:`apps/file-explorer.png`,kind:`systemDefault`,hidden:!0,darwin:{icon:`apps/finder.png`,detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)},win32:{detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)},linux:{detect:()=>`system-default`,iconPath:()=>null,args:e=>[e],open:async({path:e})=>Wa(e)}});async function Wa(e){return e}'

    node "$REPO_DIR/scripts/patch-linux-window-ui.js" "$extracted" >"$output_log" 2>&1
    assert_contains "$output_log" 'Failed to apply Linux File Manager Patch'
}

main() {
    test_common_helper_sourcing
    test_package_version_metadata_is_read_as_data
    test_package_version_metadata_trims_trailing_whitespace
    test_package_version_metadata_rejects_alphanumeric_segments
    test_package_version_metadata_rejects_too_few_segments
    test_package_identifiers_reject_path_characters
    test_package_staging_rejects_unsafe_symlinks
    test_package_staging_normalizes_payload_modes
    test_package_staging_normalizes_package_file_modes_under_private_umask
    test_package_staging_normalizes_system_directory_modes_under_private_umask
    test_deb_builder_smoke
    test_rpm_builder_smoke
    test_pacman_builder_metadata_smoke
    test_missing_input_failure
    test_make_build_app_uses_installer_download_flow_by_default
    test_installer_writes_app_version_metadata
    test_installer_rejects_alphanumeric_app_version_metadata
    test_installer_rejects_short_app_version_metadata
    test_launcher_template_sanity
    test_launcher_lifecycle_cleans_child_processes
    test_launcher_uses_updater_discovered_cli_without_path_codex
    test_launcher_falls_back_to_path_codex_when_updater_preflight_fails
    test_launcher_skips_relative_path_codex_and_uses_later_absolute_path
    test_launcher_falls_back_to_known_cli_without_updater
    test_launcher_retains_updater_printed_cli_after_preflight_failure
    test_launcher_rejects_invalid_configured_cli_before_path_fallback
    test_launcher_rejects_invalid_env_cli_with_updater_before_path_fallback
    test_launcher_help_preserves_existing_app_pid
    test_hash_workflow_opens_review_pr
    test_apple_dmg_verifier_pins_upstream_trust_inputs
    test_apple_dmg_workflow_runs_on_macos
    test_updater_service_hardening
    test_packaged_runtime_preserves_active_updater_service
    test_electron_security_inspector_flags_insecure_generated_app
    test_release_gate_requires_verified_dmg_hash
    test_release_gate_does_not_fetch_asar_with_npx
    test_release_gate_writes_checksums_and_requires_signature
    test_release_gate_removes_stale_signature_when_unsigned
    test_release_gate_fails_on_insecure_asar_contents
    test_linux_file_manager_patch_smoke
    test_linux_translucent_sidebar_default_patch_smoke
    test_linux_tray_startup_patch_smoke
    test_linux_file_manager_patch_fails_soft
    info "All script smoke tests passed"
}

main "$@"
