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
        write_webview_integrity_manifest "$install_dir" || return $?
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
import re
import sys
import urllib.parse


webview_root = pathlib.Path(sys.argv[1]).resolve()
manifest_file = pathlib.Path(sys.argv[2])
index_path = webview_root / "index.html"
STATIC_ASSET_SUFFIXES = {
    ".avif",
    ".cjs",
    ".css",
    ".gif",
    ".ico",
    ".jpeg",
    ".jpg",
    ".js",
    ".json",
    ".mjs",
    ".otf",
    ".png",
    ".svg",
    ".ttf",
    ".wasm",
    ".webp",
    ".woff",
    ".woff2",
}
STARTUP_LINK_RELS = {
    "modulepreload",
    "preload",
    "stylesheet",
}
STARTUP_SRC_TAGS = {
    "audio",
    "embed",
    "img",
    "script",
    "source",
    "track",
    "video",
}
JS_IMPORT_REF_RE = re.compile(
    r"""\bimport\s*\(\s*(?P<quote>["'])(?P<ref>[^"']+)(?P=quote)\s*\)"""
)
JS_FROM_REF_RE = re.compile(
    r"""
    \b(?:import|export)\b[^;]*?\bfrom\s*
    (?P<quote>["'])(?P<ref>[^"']+)(?P=quote)
    """,
    re.VERBOSE,
)
JS_BARE_IMPORT_REF_RE = re.compile(
    r"""\bimport\s*(?P<quote>["'])(?P<ref>[^"']+)(?P=quote)"""
)
JS_NEW_URL_REF_RE = re.compile(
    r"""\bnew\s+URL\s*\(\s*(?P<quote>["'])(?P<ref>[^"']+)(?P=quote)\s*,\s*import\.meta\.url\s*\)"""
)
JS_REQUIRE_REF_RE = re.compile(
    r"""\brequire\s*\(\s*(?P<quote>["'])(?P<ref>[^"']+)(?P=quote)\s*\)"""
)
RELATIVE_ASSET_REF_RE = re.compile(
    r"""(?P<quote>["'])(?P<ref>(?:\./|\../)[^"']+)(?P=quote)"""
)
CSS_IMPORT_REF_RE = re.compile(
    r"""@import\s+(?:url\(\s*)?(?P<quote>["']?)(?P<ref>[^"'\s;)]+)(?P=quote)\s*\)?"""
)
CSS_URL_REF_RE = re.compile(
    r"""url\(\s*(?P<quote>["']?)(?P<ref>[^"')]+)(?P=quote)\s*\)"""
)


class StartupAssetParser(html.parser.HTMLParser):
    def __init__(self):
        super().__init__()
        self.paths = set()

    def handle_starttag(self, tag, attrs):
        tag = tag.lower()
        attr_map = {
            name.lower(): value
            for name, value in attrs
            if name and value is not None
        }

        values = []
        if tag in STARTUP_SRC_TAGS:
            values.append(attr_map.get("src"))
        elif tag == "link" and self.link_rel_is_startup(attr_map.get("rel", "")):
            values.append(attr_map.get("href"))

        for value in values:
            if not value:
                continue
            normalized = normalize_asset_reference(value, "index.html", allow_plain=True)
            if normalized is not None:
                self.paths.add(normalized)

    @staticmethod
    def link_rel_is_startup(rel_value):
        rel_tokens = {token.lower() for token in rel_value.split()}
        return bool(rel_tokens & STARTUP_LINK_RELS)


def normalize_asset_reference(reference, base_relative_path, allow_plain):
    parsed = urllib.parse.urlsplit(reference.strip())
    if parsed.scheme or parsed.netloc:
        return None
    path = urllib.parse.unquote(parsed.path)
    if not path or path.startswith("#"):
        return None
    if not allow_plain and not path.startswith(("/", "./", "../")):
        return None
    if "\\" in path or any(ord(ch) < 32 for ch in path):
        raise SystemExit(f"webview startup asset has unsafe path characters: {reference}")
    if path.startswith("/"):
        combined = path.lstrip("/")
    else:
        combined = posixpath.join(posixpath.dirname(base_relative_path), path)
    normalized = posixpath.normpath(combined)
    if normalized == "." or normalized.startswith("../") or normalized == "..":
        raise SystemExit(f"webview startup asset escapes content root: {reference}")
    if "\\" in normalized or any(ord(ch) < 32 for ch in normalized):
        raise SystemExit(f"webview startup asset has unsafe path characters: {reference}")
    return normalized


def digest_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def webview_asset_path(relative_path):
    asset_path = (webview_root / relative_path).resolve()
    try:
        asset_path.relative_to(webview_root)
    except ValueError:
        raise SystemExit(f"webview startup asset escapes content root: {relative_path}")
    return asset_path


def has_static_asset_suffix(reference):
    path = urllib.parse.urlsplit(reference).path
    return pathlib.PurePosixPath(path).suffix.lower() in STATIC_ASSET_SUFFIXES


def mask_js_comments_and_strings(text):
    chars = list(text)
    index = 0
    while index < len(text):
        char = text[index]
        next_char = text[index + 1] if index + 1 < len(text) else ""

        if char == "/" and next_char == "/":
            chars[index] = " "
            chars[index + 1] = " "
            index += 2
            while index < len(text) and text[index] != "\n":
                chars[index] = " "
                index += 1
            continue

        if char == "/" and next_char == "*":
            chars[index] = " "
            chars[index + 1] = " "
            index += 2
            while index < len(text):
                if text[index] == "\n":
                    index += 1
                    continue
                if text[index] == "*" and index + 1 < len(text) and text[index + 1] == "/":
                    chars[index] = " "
                    chars[index + 1] = " "
                    index += 2
                    break
                chars[index] = " "
                index += 1
            continue

        if char in {"'", '"'}:
            quote = char
            index += 1
            while index < len(text):
                if text[index] == "\\":
                    chars[index] = " "
                    if index + 1 < len(text) and text[index + 1] != "\n":
                        chars[index + 1] = " "
                        index += 2
                    else:
                        index += 1
                    continue
                if text[index] == quote:
                    index += 1
                    break
                if text[index] != "\n":
                    chars[index] = " "
                index += 1
            continue

        if char == "`":
            chars[index] = " "
            index += 1
            while index < len(text):
                if text[index] == "\\":
                    chars[index] = " "
                    if index + 1 < len(text) and text[index + 1] != "\n":
                        chars[index + 1] = " "
                        index += 2
                    else:
                        index += 1
                    continue
                if text[index] == "`":
                    chars[index] = " "
                    index += 1
                    break
                if text[index] != "\n":
                    chars[index] = " "
                index += 1
            continue

        index += 1

    return "".join(chars)


def iter_js_dependency_references(text):
    code = mask_js_comments_and_strings(text)
    for pattern in (JS_IMPORT_REF_RE, JS_FROM_REF_RE, JS_BARE_IMPORT_REF_RE):
        for match in pattern.finditer(code):
            yield text[match.start("ref"):match.end("ref")], True, False
    for match in JS_NEW_URL_REF_RE.finditer(code):
        yield text[match.start("ref"):match.end("ref")], True, True
    for match in JS_REQUIRE_REF_RE.finditer(code):
        yield text[match.start("ref"):match.end("ref")], False, False
    for match in RELATIVE_ASSET_REF_RE.finditer(text):
        reference = match.group("ref")
        if has_static_asset_suffix(reference):
            yield reference, False, False


def iter_css_dependency_references(text):
    for pattern in (CSS_IMPORT_REF_RE, CSS_URL_REF_RE):
        for match in pattern.finditer(text):
            yield match.group("ref"), True, True


def iter_dependency_references(relative_path, asset_path):
    suffix = pathlib.PurePosixPath(relative_path).suffix.lower()
    if suffix not in {".js", ".mjs", ".cjs", ".css"}:
        return
    text = asset_path.read_text(encoding="utf-8", errors="ignore")
    if suffix == ".css":
        yield from iter_css_dependency_references(text)
    else:
        yield from iter_js_dependency_references(text)


def collect_startup_asset_graph(initial_paths):
    relative_paths = {"index.html"}
    pending = []

    def add_path(relative_path):
        if relative_path not in relative_paths:
            relative_paths.add(relative_path)
            pending.append(relative_path)

    for relative_path in sorted(initial_paths):
        add_path(relative_path)

    while pending:
        relative_path = pending.pop(0)
        asset_path = webview_asset_path(relative_path)
        if not asset_path.is_file():
            raise SystemExit(f"missing webview startup asset: {relative_path}")

        for reference, require_existing, allow_plain in iter_dependency_references(relative_path, asset_path):
            normalized = normalize_asset_reference(reference, relative_path, allow_plain)
            if normalized is None:
                continue
            dependency_path = webview_asset_path(normalized)
            if dependency_path.is_file():
                add_path(normalized)
            elif require_existing:
                raise SystemExit(f"missing webview startup asset: {normalized}")

    return relative_paths


if not index_path.is_file():
    raise SystemExit(f"missing webview startup document: {index_path}")

parser = StartupAssetParser()
parser.feed(index_path.read_text(encoding="utf-8", errors="ignore"))
relative_paths = collect_startup_asset_graph(parser.paths)

lines = []
for relative_path in sorted(relative_paths):
    asset_path = webview_asset_path(relative_path)
    if not asset_path.is_file():
        raise SystemExit(f"missing webview startup asset: {relative_path}")
    lines.append(f"{digest_file(asset_path)}  {relative_path}\n")

manifest_file.write_text("".join(lines), encoding="utf-8")
PY
    local py_rc=$?
    [ "$py_rc" -eq 0 ] || return "$py_rc"
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
