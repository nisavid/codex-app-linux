#!/usr/bin/env bash
set -euo pipefail

DEFAULT_REPO="nisavid/codex-app-linux"
DEFAULT_WORKFLOW="verify-apple-dmg.yml"
DEFAULT_RUN_PREFIX="Verify Apple DMG"

usage() {
    cat <<'EOF'
Usage:
  hash-refresh-evidence.sh sri-to-hex <sha256-SRI>
  hash-refresh-evidence.sh wait-apple-verification [options]
  hash-refresh-evidence.sh render-pr-body [options]

Commands:
  sri-to-hex
      Convert a Nix sha256-... SRI digest to lowercase SHA-256 hex.

  wait-apple-verification
      Dispatch the Apple DMG verification workflow and wait for the matching run.

  render-pr-body
      Render the hash-refresh pull request body to a file.
EOF
}

error() {
    echo "[hash-refresh-evidence][ERROR] $*" >&2
    exit 1
}

sri_to_hex() {
    local sri="$1"
    [[ "$sri" =~ ^sha256-[A-Za-z0-9+/=]{44}$ ]] || error "Invalid sha256 SRI digest: $sri"
    python3 - "$sri" <<'PY'
import base64
import sys

payload = sys.argv[1].split("-", 1)[1]
try:
    raw = base64.b64decode(payload, validate=True)
except Exception as exc:
    raise SystemExit(f"Invalid base64 payload: {exc}")
if len(raw) != 32:
    raise SystemExit(f"Decoded SHA-256 payload is {len(raw)} bytes, expected 32")
print(raw.hex())
PY
}

json_find_run() {
    local run_name="$1"
    node -e '
const fs = require("node:fs");
const runName = process.argv[2];
const input = fs.readFileSync(0, "utf8");
const runs = JSON.parse(input);
const match = runs.find((run) => run.displayTitle === runName);
if (!match) {
  process.exit(1);
}
process.stdout.write(JSON.stringify(match));
' _ "$run_name"
}

json_field() {
    local field="$1"
    node -e '
const fs = require("node:fs");
const field = process.argv[2];
const input = fs.readFileSync(0, "utf8");
const value = JSON.parse(input)[field];
if (value == null) {
  process.exit(1);
}
process.stdout.write(String(value));
' _ "$field"
}

wait_apple_verification() {
    local repo="$DEFAULT_REPO"
    local workflow="$DEFAULT_WORKFLOW"
    local ref="main"
    local dmg_url="https://persistent.oaistatic.com/codex-app-prod/Codex.dmg"
    local dmg_sha256=""
    local timeout_seconds="${CODEX_HASH_REFRESH_VERIFY_TIMEOUT_SECONDS:-1800}"
    local poll_interval_seconds="${CODEX_HASH_REFRESH_VERIFY_POLL_SECONDS:-15}"

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --repo) repo="$2"; shift 2 ;;
            --workflow) workflow="$2"; shift 2 ;;
            --ref) ref="$2"; shift 2 ;;
            --dmg-url) dmg_url="$2"; shift 2 ;;
            --dmg-sha256) dmg_sha256="$2"; shift 2 ;;
            --timeout-seconds) timeout_seconds="$2"; shift 2 ;;
            --poll-interval-seconds) poll_interval_seconds="$2"; shift 2 ;;
            *) error "Unknown wait-apple-verification option: $1" ;;
        esac
    done

    [[ "$dmg_sha256" =~ ^[0-9a-fA-F]{64}$ ]] || error "--dmg-sha256 must be a 64-character hex digest"
    [[ "$timeout_seconds" =~ ^[0-9]+$ ]] || error "--timeout-seconds must be numeric"
    [[ "$poll_interval_seconds" =~ ^[0-9]+$ ]] || error "--poll-interval-seconds must be numeric"
    [ "$poll_interval_seconds" -gt 0 ] || error "--poll-interval-seconds must be greater than zero"

    local dispatch_id="${CODEX_HASH_REFRESH_VERIFY_DISPATCH_ID:-hash-refresh-${GITHUB_RUN_ID:-manual}-${GITHUB_RUN_ATTEMPT:-0}-$$-$RANDOM}"
    local run_name="$DEFAULT_RUN_PREFIX $dmg_sha256 $dispatch_id"
    echo "[hash-refresh-evidence] Dispatching $workflow for $dmg_sha256 with dispatch ID $dispatch_id" >&2
    gh workflow run "$workflow" \
        --repo "$repo" \
        --ref "$ref" \
        -f "dmg_url=$dmg_url" \
        -f "dmg_sha256=$dmg_sha256" \
        -f "dispatch_id=$dispatch_id" \
        -f "require_dmg_gatekeeper=false" \
        -f "require_dmg_staple=false" >/dev/null

    local deadline=$((SECONDS + timeout_seconds))
    local runs_json match status conclusion url
    while [ "$SECONDS" -le "$deadline" ]; do
        runs_json="$(gh run list \
            --repo "$repo" \
            --workflow "$workflow" \
            --branch "$ref" \
            --event workflow_dispatch \
            --limit 20 \
            --json databaseId,displayTitle,status,conclusion,url,createdAt)"
        if match="$(printf '%s' "$runs_json" | json_find_run "$run_name" 2>/dev/null)"; then
            status="$(printf '%s' "$match" | json_field status)"
            conclusion="$(printf '%s' "$match" | json_field conclusion 2>/dev/null || true)"
            url="$(printf '%s' "$match" | json_field url)"
            echo "[hash-refresh-evidence] Apple verification run status: $status ${conclusion:-}" >&2
            if [ "$status" = "completed" ]; then
                [ "$conclusion" = "success" ] || error "Apple DMG verification concluded with $conclusion: $url"
                printf '%s\n' "$url"
                return 0
            fi
        fi
        sleep "$poll_interval_seconds"
    done

    error "Timed out waiting for Apple DMG verification run named '$run_name'"
}

render_pr_body() {
    local output=""
    local dmg_sri=""
    local dmg_sha256=""
    local app_version=""
    local app_build=""
    local electron_version=""
    local better_sqlite3_version=""
    local node_pty_version=""
    local verification_url=""
    local branch="bot/update-codex-dmg-hash"

    while [ "$#" -gt 0 ]; do
        case "$1" in
            --output) output="$2"; shift 2 ;;
            --dmg-sri) dmg_sri="$2"; shift 2 ;;
            --dmg-sha256) dmg_sha256="$2"; shift 2 ;;
            --app-version) app_version="$2"; shift 2 ;;
            --app-build) app_build="$2"; shift 2 ;;
            --electron-version) electron_version="$2"; shift 2 ;;
            --better-sqlite3-version) better_sqlite3_version="$2"; shift 2 ;;
            --node-pty-version) node_pty_version="$2"; shift 2 ;;
            --verification-url) verification_url="$2"; shift 2 ;;
            --branch) branch="$2"; shift 2 ;;
            *) error "Unknown render-pr-body option: $1" ;;
        esac
    done

    [ -n "$output" ] || error "--output is required"
    [ -n "$dmg_sri" ] || error "--dmg-sri is required"
    [ -n "$dmg_sha256" ] || error "--dmg-sha256 is required"
    [ -n "$app_version" ] || error "--app-version is required"
    [ -n "$app_build" ] || error "--app-build is required"
    [ -n "$electron_version" ] || error "--electron-version is required"
    [ -n "$verification_url" ] || error "--verification-url is required"

    cat > "$output" <<EOF
Refreshed Codex.dmg SRI hash to \`$dmg_sri\`.

This scheduled workflow pushes refreshed hashes to \`$branch\` and attempts to open or update a PR for maintainer review.

## Machine-produced evidence

- Official app version: \`$app_version\`
- Official app build: \`$app_build\`
- Electron version: \`$electron_version\`
- Native module pins: \`better-sqlite3=$better_sqlite3_version\`, \`node-pty=$node_pty_version\`
- Codex.dmg SRI: \`$dmg_sri\`
- Codex.dmg SHA-256: \`$dmg_sha256\`
- Apple DMG verification: passed in $verification_url
EOF
}

case "${1:-}" in
    sri-to-hex)
        [ "$#" -eq 2 ] || error "usage: $0 sri-to-hex <sha256-SRI>"
        sri_to_hex "$2"
        ;;
    wait-apple-verification)
        shift
        wait_apple_verification "$@"
        ;;
    render-pr-body)
        shift
        render_pr_body "$@"
        ;;
    --help|-h|"")
        usage
        ;;
    *)
        error "Unknown command: $1"
        ;;
esac
