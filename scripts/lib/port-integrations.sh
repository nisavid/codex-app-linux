#!/bin/bash
# port integration staging hooks.
#
# Sourced by install.sh. Do not run directly.
# shellcheck shell=bash

run_port_integration_stage_hooks() {
    local app_dir="${1:-}"
    local integration_helper="$SCRIPT_DIR/scripts/lib/port-integrations.js"
    local hooks_output=""
    local integration_id
    local hook_path

    [ -f "$integration_helper" ] || {
        warn "port integration helper not found at $integration_helper"
        return 0
    }

    if ! hooks_output="$(node "$integration_helper" --stage-hooks)"; then
        warn "port integration stage hook enumeration failed"
        return 1
    fi

    while IFS=$'\t' read -r integration_id hook_path; do
        [ -n "$integration_id" ] || continue
        [ -n "$hook_path" ] || continue
        info "Running port integration stage hook: $integration_id"
        if ! SCRIPT_DIR="$SCRIPT_DIR" INSTALL_DIR="$INSTALL_DIR" WORK_DIR="$WORK_DIR" ARCH="$ARCH" CODEX_OFFICIAL_APP_DIR="$app_dir" CODEX_UPSTREAM_APP_DIR="$app_dir" bash "$hook_path"; then
            warn "port integration stage hook failed: $integration_id"
            return 1
        fi
    done <<<"$hooks_output"
}
