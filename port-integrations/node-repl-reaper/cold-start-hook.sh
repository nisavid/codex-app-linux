#!/bin/bash
# Launcher cold-start hook: start one reaper watchdog per install. The
# watchdog self-terminates after the last electron from this install exits,
# so a stale pid file is the only restart blocker — clear it when the
# recorded pid is dead or no longer the reaper.
set -euo pipefail

app_dir="${1:?usage: cold-start hook <app-dir> <state-dir> <log-dir>}"
state_dir="${2:?usage: cold-start hook <app-dir> <state-dir> <log-dir>}"
pid_file="$state_dir/node-repl-reaper.pid"
lock_file="$pid_file.lock"
reaper="$app_dir/.codex-linux/node-repl-reaper.sh"

[ -x "$reaper" ] || exit 0

if command -v flock >/dev/null 2>&1; then
    exec 9>"$lock_file" || exit 0
    flock -x 9 || exit 0
fi

if [ -f "$pid_file" ]; then
    existing="$(cat "$pid_file" 2>/dev/null || true)"
    if [ -n "$existing" ] && [ -d "/proc/$existing" ]; then
        existing_cmdline="$(tr '\0' ' ' < "/proc/$existing/cmdline" 2>/dev/null || true)"
        case "$existing_cmdline" in
            *node-repl-reaper*) exit 0 ;;
        esac
    fi
    rm -f "$pid_file"
fi

"$reaper" "$app_dir" watch 9>&- &
printf '%s\n' "$!" > "$pid_file"
