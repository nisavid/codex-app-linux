SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
unexport BASH_ENV
unexport BASH_FUNC_module%%
unexport BASH_FUNC_ml%%

APP_DIR := $(CURDIR)/codex-app
PACKAGE_NAME := codex-app
PACMAN_PACKAGE_NAME := codex-app
NEXT_APP_DIR := $(CURDIR)/codex-app-next
REBUILD_REPORT_DIR := $(CURDIR)/dist-next/rebuild
DEV_APP_ID ?= codex-cua-lab
DEV_APP_NAME ?= Codex CUA Lab
DEV_APP_DIR ?= $(CURDIR)/$(DEV_APP_ID)-app
DEV_APP_BIN ?= $(CURDIR)/bin/$(DEV_APP_ID)
DEB_GLOB := $(CURDIR)/dist/$(PACKAGE_NAME)_*.deb
RPM_GLOB := $(CURDIR)/dist/$(PACKAGE_NAME)-*.rpm
PACMAN_GLOB := $(CURDIR)/dist/$(PACMAN_PACKAGE_NAME)-[0-9]*.pkg.tar.*

.DEFAULT_GOAL := help

.PHONY: help check test build-updater update rebuild rebuild-install inspect-upstream build-app rebuild-next run-app build-dev-app run-dev-app deb rpm pacman package apple-dmg-verify release-gate install service-enable service-status clean clean-dist clean-state

help:
	@printf '\nCodex App Linux Make Targets\n\n'
	@printf '  %-18s %s\n' "make check" "Run cargo check for codex-app-updater"
	@printf '  %-18s %s\n' "make test" "Run updater test suite"
	@printf '  %-18s %s\n' "make build-updater" "Build codex-app-updater in release mode"
	@printf '  %-18s %s\n' "make update" "Find a DMG, rebuild, and replace codex-app/ with backup"
	@printf '  %-18s %s\n' "make rebuild" "Inspect a DMG and build a side-by-side candidate"
	@printf '  %-18s %s\n' "make rebuild-install" "Find a DMG, rebuild, and install into codex-app/"
	@printf '  %-18s %s\n' "make inspect-upstream" "Inspect a DMG and write rebuild reports without changing codex-app/"
	@printf '  %-18s %s\n' "make build-app" "Run install.sh and regenerate codex-app/"
	@printf '  %-18s %s\n' "make rebuild-next" "Build a side-by-side candidate in codex-app-next/"
	@printf '  %-18s %s\n' "make run-app" "Launch the local generated Electron app from codex-app/"
	@printf '  %-18s %s\n' "make build-dev-app" "Build a side-by-side test app with a distinct app id/bin"
	@printf '  %-18s %s\n' "make run-dev-app" "Launch the side-by-side test app"
	@printf '  %-18s %s\n' "make deb" "Build the Debian package into dist/"
	@printf '  %-18s %s\n' "make rpm" "Build the RPM package into dist/ (Fedora/openSUSE)"
	@printf '  %-18s %s\n' "make pacman" "Build the pacman package into dist/ (Arch)"
	@printf '  %-18s %s\n' "make package" "Build native package (auto-detects deb, rpm, or pacman)"
	@printf '  %-18s %s\n' "make apple-dmg-verify" "Run macOS Apple trust checks for the upstream DMG"
	@printf '  %-18s %s\n' "make release-gate" "Verify DMG hash, generated app security, package metadata, checksums, and optional signature"
	@printf '  %-18s %s\n' "make install" "Install the latest generated native package"
	@printf '  %-18s %s\n' "make service-enable" "Enable and start codex-app-updater.service for the current user"
	@printf '  %-18s %s\n' "make service-status" "Show codex-app-updater.service status for the current user"
	@printf '  %-18s %s\n' "make clean" "Remove generated app, cached DMG, and dist/ artifacts"
	@printf '  %-18s %s\n' "make clean-dist" "Remove generated dist/ artifacts"
	@printf '  %-18s %s\n' "make clean-state" "Remove updater runtime state from XDG directories"
	@printf '\nVariables:\n\n'
	@printf '  %-18s %s\n' "DMG=/path/file.dmg" "Override the DMG; rebuild commands auto-find ./Codex.dmg"
	@printf '  %-18s %s\n' "NEXT_APP_DIR=..." "Override side-by-side rebuild candidate directory"
	@printf '  %-18s %s\n' "APP_DIR=..." "Override final app directory for make rebuild-install"
	@printf '  %-18s %s\n' "REBUILD_REPORT_DIR=..." "Override inspect/rebuild report output directory"
	@printf '  %-18s %s\n' "DEV_APP_ID=..." "Override side-by-side test app id/bin (default: codex-cua-lab)"
	@printf '  %-18s %s\n' "DEV_APP_NAME=..." "Override side-by-side test app display name"
	@printf '  %-18s %s\n' "PACKAGE_VERSION=..." "Override the package version for make deb / make rpm / make pacman"
	@printf '  %-18s %s\n' "DEB=/path/file.deb" "Override the .deb used by make install"
	@printf '  %-18s %s\n' "RPM=/path/file.rpm" "Override the .rpm used by make install"
	@printf '  %-18s %s\n' "PKG=/path/file.pkg.tar.zst" "Override the pacman package used by make install"
	@printf '\nExamples:\n\n'
	@printf '  %s\n' "make update"
	@printf '  %s\n' "make rebuild-install"
	@printf '  %s\n' "make rebuild DMG=/tmp/Codex.dmg"
	@printf '  %s\n' "make build-app DMG=/tmp/Codex.dmg"
	@printf '  %s\n' "make inspect-upstream DMG=/tmp/Codex.dmg"
	@printf '  %s\n' "make rebuild-next DMG=/tmp/Codex.dmg"
	@printf '  %s\n' "make run-app"
	@printf '  %s\n' "make build-dev-app"
	@printf '  %s\n' "./bin/codex-cua-lab"
	@printf '  %s\n' "make deb"
	@printf '  %s\n' "make rpm"
	@printf '  %s\n' "make pacman"
	@printf '  %s\n' "make install"
	@printf '  %s\n\n' "make service-enable"

check:
	@echo "[make] Running cargo check"
	cargo check -p codex-app-updater

test:
	@echo "[make] Running cargo test"
	cargo test -p codex-app-updater

build-updater:
	@echo "[make] Building codex-app-updater (release)"
	cargo build --release -p codex-app-updater

update: rebuild-install

rebuild:
	@echo "[make] Running safe rebuild flow"
	REBUILD_REPORT_DIR="$(REBUILD_REPORT_DIR)" \
	CODEX_NEXT_APP_DIR="$(NEXT_APP_DIR)" \
		./scripts/rebuild-candidate.sh "$(DMG)"

rebuild-install:
	@echo "[make] Running rebuild and local install flow"
	REBUILD_REPORT_DIR="$(REBUILD_REPORT_DIR)" \
	CODEX_NEXT_APP_DIR="$(NEXT_APP_DIR)" \
	CODEX_FINAL_APP_DIR="$(APP_DIR)" \
		./scripts/rebuild-candidate.sh --install "$(DMG)"

inspect-upstream:
	@echo "[make] Inspecting upstream DMG"
	./install.sh --inspect --report-dir "$(REBUILD_REPORT_DIR)" "$(DMG)"

build-app:
	@echo "[make] Regenerating codex-app from DMG"
	./install.sh "$(DMG)"

rebuild-next:
	@echo "[make] Building side-by-side rebuild candidate"
	CODEX_INSTALL_DIR="$(NEXT_APP_DIR)" \
	CODEX_PATCH_REPORT_JSON="$(REBUILD_REPORT_DIR)/patch-report.json" \
	CODEX_REBUILD_REPORT_JSON="$(REBUILD_REPORT_DIR)/rebuild-report.json" \
	REBUILD_REPORT_DIR="$(REBUILD_REPORT_DIR)" \
		./install.sh "$(DMG)"
	@echo "[make] Candidate app: $(NEXT_APP_DIR)"
	@echo "[make] Rebuild report: $(REBUILD_REPORT_DIR)/rebuild-report.json"

run-app:
	@echo "[make] Launching local Electron app"
	"$(APP_DIR)/start.sh"

build-dev-app:
	@echo "[make] Building side-by-side Electron app as $(DEV_APP_ID)"
	CODEX_APP_ID="$(DEV_APP_ID)" \
	CODEX_APP_DISPLAY_NAME="$(DEV_APP_NAME)" \
	CODEX_INSTALL_DIR="$(DEV_APP_DIR)" \
		./install.sh "$(DMG)"
	@mkdir -p "$(CURDIR)/bin"
	@ln -sfn "$(DEV_APP_DIR)/start.sh" "$(DEV_APP_BIN)"
	@echo "[make] Side-by-side launcher: $(DEV_APP_BIN)"

run-dev-app:
	@echo "[make] Launching side-by-side Electron app"
	"$(DEV_APP_BIN)"

deb: build-updater
	@echo "[make] Building Debian package"
	PACKAGE_VERSION="$(or $(PACKAGE_VERSION),)" ./scripts/build-deb.sh

rpm: build-updater
	@echo "[make] Building RPM package"
	PACKAGE_VERSION="$(or $(PACKAGE_VERSION),)" ./scripts/build-rpm.sh

pacman: build-updater
	@echo "[make] Building pacman package"
	PACKAGE_NAME="$(PACMAN_PACKAGE_NAME)" PACKAGE_VERSION="$(or $(PACKAGE_VERSION),)" ./scripts/build-pacman.sh

package: build-updater
	@echo "[make] Building native package (auto-detecting distro)"
	@if command -v makepkg >/dev/null 2>&1 && ! command -v dpkg-deb >/dev/null 2>&1; then \
		PACKAGE_NAME="$(PACMAN_PACKAGE_NAME)" PACKAGE_VERSION="$(or $(PACKAGE_VERSION),)" ./scripts/build-pacman.sh; \
	elif command -v dpkg-deb >/dev/null 2>&1; then \
		PACKAGE_VERSION="$(or $(PACKAGE_VERSION),)" ./scripts/build-deb.sh; \
	elif command -v rpmbuild >/dev/null 2>&1; then \
		PACKAGE_VERSION="$(or $(PACKAGE_VERSION),)" ./scripts/build-rpm.sh; \
	else \
		echo "[make] No supported packaging tool found. Install dpkg-dev (Debian), rpm-build (Fedora), or pacman (Arch)." >&2; \
		exit 1; \
	fi

apple-dmg-verify:
	@echo "[make] Running Apple DMG verification"
	@if [ -n "$(DMG)" ]; then \
		./scripts/verify-apple-dmg.sh --dmg "$(DMG)"; \
	else \
		./scripts/verify-apple-dmg.sh; \
	fi

release-gate:
	@echo "[make] Running release gate"
	./scripts/release-gate.sh

install:
	@echo "[make] Installing latest native package"
	@if command -v pacman >/dev/null 2>&1 && ! command -v dpkg >/dev/null 2>&1; then \
		pkg="$${PKG:-$$(ls -1 $(PACMAN_GLOB) 2>/dev/null | sort -V | tail -n 1)}"; \
		if [ -z "$$pkg" ]; then \
			echo "[make] No pacman package found. Run 'make pacman' first." >&2; exit 1; \
		fi; \
		echo "[make] Installing $$pkg"; \
		sudo pacman -U --noconfirm "$$pkg"; \
	elif command -v dpkg >/dev/null 2>&1; then \
		deb="$${DEB:-$$(ls -1 $(DEB_GLOB) 2>/dev/null | sort -V | tail -n 1)}"; \
		if [ -z "$$deb" ]; then \
			echo "[make] No Debian package found. Run 'make deb' first." >&2; exit 1; \
		fi; \
		echo "[make] Installing $$deb"; \
		sudo dpkg -i "$$deb"; \
	elif command -v zypper >/dev/null 2>&1; then \
		rpm="$${RPM:-$$(ls -1 $(RPM_GLOB) 2>/dev/null | sort -V | tail -n 1)}"; \
		if [ -z "$$rpm" ]; then \
			echo "[make] No RPM package found. Run 'make rpm' first." >&2; exit 1; \
		fi; \
		if [ ! -f "$$rpm" ]; then \
			echo "[make] RPM must point to a local .rpm file when using zypper." >&2; exit 1; \
		fi; \
		case "$$rpm" in *.rpm) ;; *) echo "[make] RPM must point to a local .rpm file when using zypper." >&2; exit 1 ;; esac; \
		rpm="$$(readlink -f "$$rpm")"; \
		dist_dir="$$(readlink -f dist)"; \
		set -- --non-interactive; \
		case "$$rpm" in "$$dist_dir"/*.rpm) set -- "$$@" --allow-unsigned-rpm ;; esac; \
		echo "[make] Installing $$rpm"; \
		sudo zypper "$$@" install -y "$$rpm"; \
	elif command -v rpm >/dev/null 2>&1; then \
		rpm="$${RPM:-$$(ls -1 $(RPM_GLOB) 2>/dev/null | sort -V | tail -n 1)}"; \
		if [ -z "$$rpm" ]; then \
			echo "[make] No RPM package found. Run 'make rpm' first." >&2; exit 1; \
		fi; \
		echo "[make] Installing $$rpm"; \
		sudo rpm -Uvh "$$rpm"; \
	else \
		echo "[make] No supported package manager found (dpkg, rpm, zypper, or pacman)." >&2; exit 1; \
	fi

service-enable:
	@echo "[make] Enabling and starting codex-app-updater.service"
	systemctl --user daemon-reload
	systemctl --user enable --now codex-app-updater.service

service-status:
	@echo "[make] Showing codex-app-updater.service status"
	systemctl --user status codex-app-updater.service --no-pager

clean:
	@echo "[make] Removing generated app, cached DMG, dist/, and rebuild artifacts"
	rm -rf "$(CURDIR)/codex-app" "$(CURDIR)/Codex.dmg" "$(CURDIR)/dist" "$(NEXT_APP_DIR)" "$(REBUILD_REPORT_DIR)"

clean-dist:
	@echo "[make] Removing dist/ and rebuild reports"
	rm -rf "$(CURDIR)/dist" "$(REBUILD_REPORT_DIR)"

clean-state:
	@echo "[make] Removing updater runtime state"
	rm -rf \
		"$$HOME/.config/codex-app-updater" \
		"$$HOME/.local/state/codex-app-updater" \
		"$$HOME/.cache/codex-app-updater"
