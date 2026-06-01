#!/usr/bin/env bash
set -Eeuo pipefail

warn() {
    echo "WARN: $*" >&2
}

candidate_skill_sources=()

app_dir="${CODEX_LINUX_APP_DIR:-${1:-}}"
if [ -n "$app_dir" ]; then
    candidate_skill_sources+=("$app_dir/.codex-linux/integrations/agent-workspace/skills/agent-workspace-linux/SKILL.md")
fi

if [ -n "${CODEX_PORT_INTEGRATIONS_DIR:-}" ]; then
    candidate_skill_sources+=("$CODEX_PORT_INTEGRATIONS_DIR/agent-workspace/skills/agent-workspace-linux/SKILL.md")
fi

skill_source=""
for candidate in "${candidate_skill_sources[@]}"; do
    if [ -f "$candidate" ]; then
        skill_source="$candidate"
        break
    fi
done

if [ -z "$skill_source" ]; then
    warn "Agent Workspaces skill source not found in staged app resources; skipping skill install"
    exit 0
fi

codex_home="${CODEX_HOME:-}"
if [ -z "$codex_home" ]; then
    if [ -z "${HOME:-}" ]; then
        warn "CODEX_HOME is not set and HOME is unavailable; skipping Agent Workspaces skill install"
        exit 0
    fi
    codex_home="$HOME/.codex"
fi

target_dir="$codex_home/skills/agent-workspace-linux"
target_skill="$target_dir/SKILL.md"

if ! mkdir -p "$target_dir"; then
    warn "Could not create Agent Workspaces skill directory at $target_dir"
    exit 0
fi

if [ -f "$target_skill" ] && cmp -s "$skill_source" "$target_skill"; then
    echo "Agent Workspaces skill already current at $target_skill" >&2
    exit 0
fi

if ! install -m 0644 "$skill_source" "$target_skill"; then
    warn "Could not install Agent Workspaces skill to $target_skill"
    exit 0
fi

echo "Installed Agent Workspaces skill to $target_skill" >&2
