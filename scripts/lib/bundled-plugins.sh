#!/bin/bash
# Bundled-plugin staging — Linux Computer Use backend build, plugin manifest, marketplace.
#
# Sourced by install.sh. Do not run directly.
# shellcheck shell=bash

# ---- Install Linux-safe bundled plugin resources ----
find_cargo_for_linux_computer_use() {
    if command -v cargo >/dev/null 2>&1; then
        command -v cargo
        return 0
    fi

    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        echo "$HOME/.cargo/bin/cargo"
        return 0
    fi

    return 1
}

build_linux_computer_use_backend() {
    local crate_dir="$SCRIPT_DIR/computer-use-linux"
    local backend_binary="$SCRIPT_DIR/target/release/codex-computer-use-linux"
    local cargo_cmd=""

    if [ ! -d "$crate_dir" ]; then
        warn "Linux Computer Use backend source not found at $crate_dir"
        return 1
    fi

    if ! cargo_cmd="$(find_cargo_for_linux_computer_use)"; then
        warn "cargo not found; Linux Computer Use plugin will be unavailable"
        return 1
    fi

    info "Building Linux Computer Use backend..."
    if ! (cd "$SCRIPT_DIR" && "$cargo_cmd" build --release -p codex-computer-use-linux >&2); then
        warn "Failed to build Linux Computer Use backend"
        return 1
    fi

    [ -x "$backend_binary" ] || {
        warn "Linux Computer Use backend binary missing after build: $backend_binary"
        return 1
    }

    echo "$backend_binary"
}

stage_linux_computer_use_plugin() {
    local target_plugins="$1"
    local plugin_template="$SCRIPT_DIR/plugins/openai-bundled/plugins/computer-use"
    local backend_binary=""
    local target_plugin="$target_plugins/computer-use"

    if [ ! -d "$plugin_template" ]; then
        warn "Linux Computer Use plugin template not found at $plugin_template"
        return 1
    fi

    if ! backend_binary="$(build_linux_computer_use_backend)"; then
        return 1
    fi

    rm -rf "$target_plugin"
    mkdir -p "$target_plugin"
    cp -R "$plugin_template/." "$target_plugin/"
    mkdir -p "$target_plugin/bin"
    cp "$backend_binary" "$target_plugin/bin/codex-computer-use-linux"
    chmod 0755 "$target_plugin/bin/codex-computer-use-linux"

    if [ -f "$ICON_SOURCE" ]; then
        mkdir -p "$target_plugin/assets"
        cp "$ICON_SOURCE" "$target_plugin/assets/app-icon.png"
    fi

    find "$target_plugin" \( -name '*:com.apple.*' -o -name '.gitkeep' \) -delete
    return 0
}

is_elf_executable() {
    local file="$1"
    python3 - "$file" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
try:
    sys.exit(0 if path.read_bytes()[:4] == b"\x7fELF" else 1)
except OSError:
    sys.exit(1)
PY
}

install_linux_executable_resource() {
    local source="$1"
    local destination="$2"
    local label="$3"

    if [ ! -f "$source" ]; then
        warn "Browser Use $label not found in upstream resources; skipping"
        return 1
    fi

    if ! is_elf_executable "$source"; then
        warn "Browser Use $label is not a Linux executable; skipping"
        return 1
    fi

    install -m 0755 "$source" "$destination"
}

remove_macos_sidecar_files() {
    local root="$1"
    find "$root" -type f -name '*:com.apple.*' -delete
}

write_bundled_plugins_marketplace() {
    local source="$1"
    local destination="$2"
    local include_browser="$3"
    local include_computer_use="$4"

    node - "$source" "$destination" "$include_browser" "$include_computer_use" <<'NODE'
const fs = require("fs");
const path = require("path");

const sourcePath = process.argv[2];
const destinationPath = process.argv[3];
const includeBrowser = process.argv[4] === "1";
const includeComputerUse = process.argv[5] === "1";
const marketplace = JSON.parse(fs.readFileSync(sourcePath, "utf8"));
const sourcePlugins = marketplace.plugins || [];
const plugins = [];

if (includeBrowser) {
  const browserUse = sourcePlugins.find((plugin) => plugin.name === "browser-use");
  if (browserUse == null) {
    throw new Error("Bundled marketplace does not contain browser-use plugin");
  }
  plugins.push(browserUse);
}

if (includeComputerUse) {
  plugins.push({
    name: "computer-use",
    source: {
      source: "local",
      path: "./plugins/computer-use",
    },
    policy: {
      installation: "AVAILABLE",
      authentication: "ON_INSTALL",
    },
    category: "Productivity",
  });
}

marketplace.plugins = plugins;
fs.mkdirSync(path.dirname(destinationPath), { recursive: true });
fs.writeFileSync(destinationPath, `${JSON.stringify(marketplace, null, 2)}\n`);
NODE
}

install_bundled_plugin_resources() {
    local app_dir="$1"
    local upstream_resources="$app_dir/Contents/Resources"
    local source_marketplace="$upstream_resources/plugins/openai-bundled/.agents/plugins/marketplace.json"
    local source_plugin="$upstream_resources/plugins/openai-bundled/plugins/browser-use"
    local resources_dir="$INSTALL_DIR/resources"
    local bundled_plugins_dir="$resources_dir/plugins/openai-bundled"
    local include_browser=0
    local include_computer_use=0

    if [ ! -f "$source_marketplace" ]; then
        warn "Bundled plugin marketplace not found in upstream app; skipping bundled plugins"
        return 0
    fi

    mkdir -p "$bundled_plugins_dir/plugins" "$bundled_plugins_dir/.agents/plugins"

    if [ -d "$source_plugin" ]; then
        rm -rf "$bundled_plugins_dir/plugins/browser-use"
        cp -R "$source_plugin" "$bundled_plugins_dir/plugins/browser-use"
        remove_macos_sidecar_files "$bundled_plugins_dir/plugins/browser-use"
        include_browser=1
    else
        warn "Browser Use bundled plugin resources not found in upstream app; skipping Browser Use"
    fi

    if stage_linux_computer_use_plugin "$bundled_plugins_dir/plugins"; then
        include_computer_use=1
    else
        warn "Linux Computer Use plugin will be unavailable"
    fi

    if [ "$include_browser" -eq 0 ] && [ "$include_computer_use" -eq 0 ]; then
        warn "No Linux-safe bundled plugins were staged"
        return 0
    fi

    write_bundled_plugins_marketplace "$source_marketplace" "$bundled_plugins_dir/.agents/plugins/marketplace.json" "$include_browser" "$include_computer_use"

    install_linux_executable_resource "$upstream_resources/node" "$resources_dir/node" "node runtime" || true
    install_linux_executable_resource "$upstream_resources/node_repl" "$resources_dir/node_repl" "node_repl runtime" || true

    info "Linux-safe bundled plugins installed"
}
