#!/bin/bash
# Webview asset extraction and patched app.asar install into the codex-app/ tree.
#
# Sourced by install.sh. Do not run directly.
# shellcheck shell=bash

# ---- Extract webview files ----
extract_webview() {
    local install_dir="$1"
    local target_webview="$install_dir/content/webview"
    mkdir -p "$target_webview"

    # Webview files are inside the extracted asar at webview/
    local asar_extracted="$WORK_DIR/app-extracted"
    if [ -d "$asar_extracted/webview" ]; then
        cp -aT "$asar_extracted/webview" "$target_webview"
        # Replace transparent startup background with an opaque color for Linux.
        # The official OpenAI app bundle relies on macOS vibrancy for the transparent effect;
        # on Linux the transparent background causes flickering.
        local webview_index="$target_webview/index.html"
        if [ -f "$webview_index" ]; then
            sed -i 's/--startup-background: transparent/--startup-background: #1e1e1e/' "$webview_index"
        fi
        write_webview_integrity_manifest "$install_dir"
        info "Webview files copied"
    else
        warn "Webview directory not found in asar — app may not work"
    fi
}

write_webview_integrity_manifest() {
    local install_dir="$1"
    local target_webview="$install_dir/content/webview"
    local manifest_dir="$install_dir/.codex-linux"
    local manifest_file="$manifest_dir/webview-integrity.sha256"

    mkdir -p "$manifest_dir"
    python3 - "$target_webview" "$manifest_file" <<'PY'
import hashlib
import html.parser
import pathlib
import posixpath
import sys
import urllib.parse


webview_root = pathlib.Path(sys.argv[1]).resolve()
manifest_file = pathlib.Path(sys.argv[2])
index_path = webview_root / "index.html"


class StartupAssetParser(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.paths = set()

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name not in {"href", "src"} or not value:
                continue
            parsed = urllib.parse.urlsplit(value)
            if parsed.scheme or parsed.netloc:
                continue
            path = urllib.parse.unquote(parsed.path)
            if path.startswith("/"):
                path = path.lstrip("/")
            if not path:
                continue
            normalized = posixpath.normpath(path)
            if normalized == "." or normalized.startswith("../") or normalized == "..":
                raise SystemExit(f"webview startup asset escapes content root: {value}")
            if "\\" in normalized or any(ord(ch) < 32 for ch in normalized):
                raise SystemExit(f"webview startup asset has unsafe path characters: {value}")
            self.paths.add(normalized)


def digest_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


if not index_path.is_file():
    raise SystemExit(f"missing webview startup document: {index_path}")

parser = StartupAssetParser()
parser.feed(index_path.read_text(encoding="utf-8", errors="ignore"))
relative_paths = {"index.html", *parser.paths}

lines = []
for relative_path in sorted(relative_paths):
    asset_path = (webview_root / relative_path).resolve()
    try:
        asset_path.relative_to(webview_root)
    except ValueError:
        raise SystemExit(f"webview startup asset escapes content root: {relative_path}")
    if not asset_path.is_file():
        raise SystemExit(f"missing webview startup asset: {relative_path}")
    lines.append(f"{digest_file(asset_path)}  {relative_path}\n")

manifest_file.write_text("".join(lines), encoding="utf-8")
PY
    info "Webview integrity manifest written"
}

# ---- Install app.asar ----
install_app() {
    cp "$WORK_DIR/app.asar" "$INSTALL_DIR/resources/"
    if [ -d "$WORK_DIR/app.asar.unpacked" ]; then
        cp -r "$WORK_DIR/app.asar.unpacked" "$INSTALL_DIR/resources/"
    fi
    info "app.asar installed"
}
