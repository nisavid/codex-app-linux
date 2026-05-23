#!/bin/sh

codex_app_refresh_desktop_database() {
    codex_app_db_dir="${1:-}"
    [ -n "$codex_app_db_dir" ] || return 0

    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$codex_app_db_dir" >/dev/null 2>&1 || true
    fi
}

codex_app_write_user_local_entry() {
    codex_app_template_path="${1:?missing desktop template path}"
    codex_app_target_path="${2:?missing desktop target path}"
    codex_app_home_dir="${3:?missing home directory}"

    mkdir -p "$(dirname "$codex_app_target_path")"
    sed "s|@HOME@|${codex_app_home_dir}|g" \
        "$codex_app_template_path" > "$codex_app_target_path"
    chmod 0644 "$codex_app_target_path"
    codex_app_refresh_desktop_database "$(dirname "$codex_app_target_path")"
}

codex_app_entry_has_sidebar_mime() {
    grep -Eq '^MimeType=.*x-scheme-handler/codex-browser-sidebar([;]|$)' "$1"
}

codex_app_entry_has_new_window_action() {
    grep -Eq '^Actions=.*new-window([;]|$)' "$1" &&
        grep -Eq '^\[Desktop Action new-window\]$' "$1"
}

codex_app_entry_is_legacy_generated() {
    codex_app_file="${1:?missing desktop entry path}"
    [ -f "$codex_app_file" ] || return 1

    grep -Eq '^Name=(Codex App|Codex Desktop)$' "$codex_app_file" || return 1
    grep -Eq '(^Exec=.*codex-(app|desktop)|^TryExec=.*codex-(app|desktop)|^Icon=codex-(app|desktop)$)' \
        "$codex_app_file" || return 1

    if grep -Eq 'codex-(app|desktop)-open-next|^Actions=NewWindow([;]|$)|^\[Desktop Action NewWindow\]$|^Actions=NewInstance([;]|$)|^\[Desktop Action NewInstance\]$' \
        "$codex_app_file"; then
        return 0
    fi

    if ! codex_app_entry_has_sidebar_mime "$codex_app_file"; then
        return 0
    fi

    if ! codex_app_entry_has_new_window_action "$codex_app_file"; then
        return 0
    fi

    return 1
}

codex_app_next_backup_path() {
    codex_app_backup_target="${1:?missing desktop entry path}.bak"
    codex_app_backup_index=0

    while [ -e "$codex_app_backup_target" ]; do
        codex_app_backup_index=$((codex_app_backup_index + 1))
        codex_app_backup_target="${1}.bak.${codex_app_backup_index}"
    done

    printf '%s\n' "$codex_app_backup_target"
}

codex_app_repair_shadow_entry() {
    codex_app_target_path="${1:?missing desktop entry path}"
    codex_app_backup_target=""

    if ! codex_app_entry_is_legacy_generated "$codex_app_target_path"; then
        return 1
    fi

    codex_app_backup_target="$(codex_app_next_backup_path "$codex_app_target_path")"
    mv "$codex_app_target_path" "$codex_app_backup_target"
    codex_app_refresh_desktop_database "$(dirname "$codex_app_target_path")"
}

codex_app_repair_system_package_shadow_entries() {
    codex_app_package_name="${1:-codex-app}"
    codex_app_target_file="${codex_app_package_name}.desktop"

    if ! command -v runuser >/dev/null 2>&1 || ! command -v getent >/dev/null 2>&1; then
        return 0
    fi

    for codex_app_runtime_dir in /run/user/*; do
        [ -d "$codex_app_runtime_dir" ] || continue

        codex_app_uid="$(basename "$codex_app_runtime_dir")"
        case "$codex_app_uid" in
            ''|*[!0-9]*|0)
                continue
                ;;
        esac

        codex_app_passwd_entry="$(getent passwd "$codex_app_uid" || true)"
        [ -n "$codex_app_passwd_entry" ] || continue

        codex_app_user_name="$(printf '%s\n' "$codex_app_passwd_entry" | cut -d: -f1)"
        codex_app_home_dir="$(printf '%s\n' "$codex_app_passwd_entry" | cut -d: -f6)"
        [ -n "$codex_app_user_name" ] || continue
        [ -n "$codex_app_home_dir" ] || continue
        [ "$codex_app_home_dir" != "/" ] || continue

        codex_app_user_entry="$codex_app_home_dir/.local/share/applications/$codex_app_target_file"
        if ! codex_app_entry_is_legacy_generated "$codex_app_user_entry"; then
            continue
        fi

        codex_app_backup_target="$(codex_app_next_backup_path "$codex_app_user_entry")"
        runuser -u "$codex_app_user_name" -- mv \
            "$codex_app_user_entry" "$codex_app_backup_target" >/dev/null 2>&1 || true
        runuser -u "$codex_app_user_name" -- sh -c '
            if command -v update-desktop-database >/dev/null 2>&1; then
                update-desktop-database "$1" >/dev/null 2>&1 || true
            fi
        ' sh "$codex_app_home_dir/.local/share/applications" >/dev/null 2>&1 || true
    done
}
