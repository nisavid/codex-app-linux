#!/bin/bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
. "$REPO_DIR/scripts/lib/package-common.sh"
APP_DIR="${APP_DIR_OVERRIDE:-$REPO_DIR/codex-app}"
DIST_DIR="${DIST_DIR_OVERRIDE:-$REPO_DIR/dist}"
PKGBUILD_TEMPLATE="$REPO_DIR/packaging/linux/PKGBUILD.template"
INSTALL_HOOKS="$REPO_DIR/packaging/linux/codex-app.install"
DESKTOP_TEMPLATE="$REPO_DIR/packaging/linux/codex-app.desktop"
SERVICE_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater.service"
USER_SERVICE_HELPER_TEMPLATE="$REPO_DIR/packaging/linux/codex-app-updater-user-service.sh"
ICON_SOURCE="$REPO_DIR/assets/codex.png"
PACKAGED_RUNTIME_TEMPLATE="$REPO_DIR/packaging/linux/codex-packaged-runtime.sh"

PACKAGE_NAME="${PACKAGE_NAME:-codex-app}"
PACKAGE_VERSION="${PACKAGE_VERSION:-$(default_package_version)}"
PACKAGE_PROVIDES="${PACKAGE_PROVIDES:-codex-desktop}"
PACKAGE_CONFLICTS="${PACKAGE_CONFLICTS:-codex-desktop}"
UPDATER_BINARY_SOURCE="${UPDATER_BINARY_SOURCE:-$REPO_DIR/target/release/codex-app-updater}"
UPDATER_SERVICE_SOURCE="${UPDATER_SERVICE_SOURCE:-$SERVICE_TEMPLATE}"
PACKAGED_RUNTIME_SOURCE="${PACKAGED_RUNTIME_SOURCE:-$PACKAGED_RUNTIME_TEMPLATE}"

map_arch() {
	case "$(uname -m)" in
	x86_64) echo "x86_64" ;;
	aarch64) echo "aarch64" ;;
	*) error "Unsupported architecture: $(uname -m)" ;;
	esac
}

# Arch pkgver may contain '+', so keep the caller-provided commitish suffix in
# pkgver. pkgrel is reserved for distro/package rebuilds of the same upstream
# app version.
pacman_version_parts() {
	PACMAN_PKGVER="$PACKAGE_VERSION"
	PACMAN_PKGREL="1"
}

main() {
	ensure_app_layout
	ensure_file_exists "$PKGBUILD_TEMPLATE" "PKGBUILD template"
	ensure_file_exists "$INSTALL_HOOKS" "install hooks"
	ensure_file_exists "$DESKTOP_TEMPLATE" "desktop template"
	ensure_file_exists "$ICON_SOURCE" "icon"
	ensure_file_exists "$PACKAGED_RUNTIME_SOURCE" "packaged launcher runtime helper"
	if package_with_updater_enabled; then
		ensure_file_exists "$UPDATER_SERVICE_SOURCE" "updater service template"
		ensure_file_exists "$USER_SERVICE_HELPER_TEMPLATE" "updater user service helper"
	else
		info "Building package without codex-app-updater (PACKAGE_WITH_UPDATER=0)"
	fi
	command -v makepkg >/dev/null 2>&1 || error "makepkg is required (part of pacman)"

	if [ "$(id -u)" -eq 0 ]; then
		error "makepkg cannot run as root. Run this script as a regular user."
	fi

	if package_with_updater_enabled; then
		ensure_updater_binary
	fi

	local arch
	arch="$(map_arch)"
	pacman_version_parts

	local build_root
	build_root="$(mktemp -d)"
	# shellcheck disable=SC2064
	trap "rm -rf '$build_root'" EXIT

	local staging_root="$build_root/staging"

	stage_common_package_files "$staging_root"
	stage_optional_update_builder_bundle "$staging_root"
	write_launcher_stub "$staging_root"

	local package_name
	local package_provides
	local package_conflicts
	local pacman_pkgver
	local pacman_pkgrel
	local staging_dir
	local arch_replacement
	package_name="$(sed_escape_replacement "$PACKAGE_NAME")"
	package_provides="$(sed_escape_replacement "$PACKAGE_PROVIDES")"
	package_conflicts="$(sed_escape_replacement "$PACKAGE_CONFLICTS")"
	pacman_pkgver="$(sed_escape_replacement "$PACMAN_PKGVER")"
	pacman_pkgrel="$(sed_escape_replacement "$PACMAN_PKGREL")"
	staging_dir="$(sed_escape_replacement "$staging_root")"
	arch_replacement="$(sed_escape_replacement "$arch")"

	local pacman_updater_depends=""
	if package_with_updater_enabled; then
		pacman_updater_depends="    'p7zip'
    'polkit'
    'curl'
    'unzip'
    'gcc'
    'make'"
	fi
	sed \
		-e "s/__PACKAGE_NAME__/$package_name/g" \
		-e "s/__PACKAGE_PROVIDES__/$package_provides/g" \
		-e "s/__PACKAGE_CONFLICTS__/$package_conflicts/g" \
		-e "s/__PKGVER__/$pacman_pkgver/g" \
		-e "s/__PKGREL__/$pacman_pkgrel/g" \
		-e "s|__STAGING_DIR__|$staging_dir|g" \
		-e "s/__ARCH__/$arch_replacement/g" \
		"$PKGBUILD_TEMPLATE" | \
	AWK_PACMAN_UPDATER_DEPENDS="$pacman_updater_depends" \
	awk '
		function emit_env(name) {
			if (ENVIRON[name] != "") {
				print ENVIRON[name]
			}
		}
		{
			if ($0 == "__PACMAN_UPDATER_DEPENDS__") { emit_env("AWK_PACMAN_UPDATER_DEPENDS"); next }
			print
		}
	' >"$build_root/PKGBUILD"

	local updater_service_preamble=""
	local updater_post_install=""
	local updater_pre_remove="    :"
	local updater_post_remove="    :"
	if package_with_updater_enabled; then
		updater_service_preamble="SERVICE_HELPER=\"/usr/lib/$PACKAGE_NAME/update-builder/packaging/linux/codex-app-updater-user-service.sh\"
if [ -f \"\$SERVICE_HELPER\" ]; then
    # shellcheck source=/usr/lib/$PACKAGE_NAME/update-builder/packaging/linux/codex-app-updater-user-service.sh
    . \"\$SERVICE_HELPER\"
fi"
		updater_post_install="    if [ -f \"\$SERVICE_HELPER\" ]; then
        codex_ensure_user_service_running || true
    fi"
		updater_pre_remove="    if [ -f \"\$SERVICE_HELPER\" ]; then
        codex_cleanup_user_service stop || true
        codex_cleanup_user_service disable || true
    fi"
		updater_post_remove="    if [ -f \"\$SERVICE_HELPER\" ]; then
        codex_reload_user_managers || true
    fi"
		AWK_PACKAGE_NAME="$PACKAGE_NAME" \
		AWK_UPDATER_SERVICE_PREAMBLE="$updater_service_preamble" \
		AWK_UPDATER_POST_INSTALL="$updater_post_install" \
		AWK_UPDATER_PRE_REMOVE="$updater_pre_remove" \
		AWK_UPDATER_POST_REMOVE="$updater_post_remove" \
		awk '
			function emit_env(name) {
				if (ENVIRON[name] != "") {
					print ENVIRON[name]
				}
			}
			{
				if ($0 == "__UPDATER_SERVICE_PREAMBLE__") { emit_env("AWK_UPDATER_SERVICE_PREAMBLE"); next }
				if ($0 == "__UPDATER_POST_INSTALL__") { emit_env("AWK_UPDATER_POST_INSTALL"); next }
				if ($0 == "__UPDATER_PRE_REMOVE__") { emit_env("AWK_UPDATER_PRE_REMOVE"); next }
				if ($0 == "__UPDATER_POST_REMOVE__") { emit_env("AWK_UPDATER_POST_REMOVE"); next }
				gsub(/\/opt\/codex-app/, "/opt/" ENVIRON["AWK_PACKAGE_NAME"])
				gsub(/\/usr\/lib\/codex-app/, "/usr/lib/" ENVIRON["AWK_PACKAGE_NAME"])
				print
			}
		' "$INSTALL_HOOKS" >"$build_root/${PACKAGE_NAME}.install"
	else
		write_no_updater_pacman_install_hooks "$build_root/${PACKAGE_NAME}.install"
	fi

	mkdir -p "$DIST_DIR"
	info "Building ${PACKAGE_NAME}-${PACMAN_PKGVER}-${PACMAN_PKGREL}-${arch}.pkg.tar.zst"

	# Build the package; --nodeps skips dependency checks at build time (they
	# are enforced by pacman at install time), and --skipinteg is needed
	# because we have no remote sources to verify.
	(cd "$build_root" && PKGDEST="$DIST_DIR" makepkg -f --nodeps --skipinteg 2>&1) >&2

	local pkg_file=""
	pkg_file="$(find "$DIST_DIR" \( -name "${PACKAGE_NAME}-${PACMAN_PKGVER}-*.pkg.tar.zst" \
		-o -name "${PACKAGE_NAME}-${PACMAN_PKGVER}-*.pkg.tar.xz" \) \
		-print -quit 2>/dev/null || true)"
	[ -f "$pkg_file" ] || error "makepkg did not produce a package"

	if command -v pacman >/dev/null 2>&1; then
		info "Inspecting package metadata"
		pacman -Qip "$pkg_file" >&2
		info "Inspecting package contents"
		pacman -Qlp "$pkg_file" >&2
	fi

	ln -sfn "$(basename "$pkg_file")" "$DIST_DIR/${PACKAGE_NAME}-latest.pkg.tar.zst"

	info "Built package: $pkg_file"
	printf '%s\n' "$pkg_file"
}

main "$@"
