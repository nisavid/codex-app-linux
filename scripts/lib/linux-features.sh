#!/bin/bash
# Linux feature staging hooks.
#
# Sourced by install.sh. Do not run directly.
# shellcheck shell=bash

run_linux_feature_stage_hooks() {
    local app_dir="${1:-}"
    local feature_helper="$SCRIPT_DIR/scripts/lib/linux-features.js"
    local hooks_dir=""
    local hooks_file=""
    local feature_id
    local hook_path

    [ -f "$feature_helper" ] || {
        warn "Linux feature helper not found at $feature_helper"
        return 0
    }

    hooks_dir="${WORK_DIR:-/tmp}"
    [ -d "$hooks_dir" ] || hooks_dir="/tmp"
    hooks_file="$(mktemp "$hooks_dir/codex-linux-feature-hooks.XXXXXX")" || return 1
    if ! node "$feature_helper" --stage-hooks >"$hooks_file"; then
        warn "Linux feature stage hook enumeration failed"
        rm -f "$hooks_file"
        return 1
    fi

    while IFS=$'\t' read -r feature_id hook_path; do
        [ -n "$feature_id" ] || continue
        [ -n "$hook_path" ] || continue
        info "Running Linux feature stage hook: $feature_id"
        if ! SCRIPT_DIR="$SCRIPT_DIR" INSTALL_DIR="$INSTALL_DIR" WORK_DIR="$WORK_DIR" ARCH="$ARCH" CODEX_UPSTREAM_APP_DIR="$app_dir" bash "$hook_path"; then
            warn "Linux feature stage hook failed: $feature_id"
            rm -f "$hooks_file"
            return 1
        fi
    done <"$hooks_file"
    rm -f "$hooks_file"
}
