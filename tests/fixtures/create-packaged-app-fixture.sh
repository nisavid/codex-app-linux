#!/usr/bin/env bash
set -Eeuo pipefail

app_dir="${1:-codex-app}"

mkdir -p \
    "$app_dir/content/webview" \
    "$app_dir/resources/node-runtime/bin" \
    "$app_dir/resources/node-runtime/lib/node_modules/npm/bin"

printf '%s\n' '#!/usr/bin/env bash' 'echo "codex desktop fixture"' > "$app_dir/start.sh"
chmod +x "$app_dir/start.sh"

printf '%s\n' '<!doctype html><title>Codex fixture</title>' > "$app_dir/content/webview/index.html"

for binary in node npm-cli.js npx-cli.js; do
    cat > "$app_dir/resources/node-runtime/bin/$binary" <<'SCRIPT'
#!/usr/bin/env bash
case "$(basename "$0")" in
    node) echo v22.22.2 ;;
    *) echo 10.9.7 ;;
esac
SCRIPT
    chmod +x "$app_dir/resources/node-runtime/bin/$binary"
done

mv "$app_dir/resources/node-runtime/bin/npm-cli.js" \
    "$app_dir/resources/node-runtime/lib/node_modules/npm/bin/npm-cli.js"
mv "$app_dir/resources/node-runtime/bin/npx-cli.js" \
    "$app_dir/resources/node-runtime/lib/node_modules/npm/bin/npx-cli.js"
ln -s ../lib/node_modules/npm/bin/npm-cli.js "$app_dir/resources/node-runtime/bin/npm"
ln -s ../lib/node_modules/npm/bin/npx-cli.js "$app_dir/resources/node-runtime/bin/npx"
