#!/usr/bin/env bash
set -euo pipefail

chrome_plugin="$INSTALL_DIR/resources/plugins/openai-bundled/plugins/chrome"
patcher="$SCRIPT_DIR/port-integrations/thorium-chrome-plugin/patch-chrome-plugin.js"
manifest_paths_dir="$INSTALL_DIR/.codex-linux"
manifest_paths_file="$manifest_paths_dir/chrome-native-host-manifest-paths"

if [ ! -d "$chrome_plugin" ]; then
    echo "WARN: Chrome plugin not found; skipping Thorium Chrome plugin patch" >&2
    exit 0
fi

mkdir -p "$manifest_paths_dir"
touch "$manifest_paths_file"
if ! grep -Fxq ".config/thorium/NativeMessagingHosts" "$manifest_paths_file"; then
    printf '%s\n' ".config/thorium/NativeMessagingHosts" >> "$manifest_paths_file"
fi

node "$patcher" "$chrome_plugin" >&2
