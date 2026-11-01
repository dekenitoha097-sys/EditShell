#!/usr/bin/env bash
set -euo pipefail

home="${HOME:?HOME must be set}"
bin_file="$home/.local/bin/editshell"
data_dir="${XDG_DATA_HOME:-"$home/.local/share"}"
desktop_file="$data_dir/applications/io.github.editshell.EditShell.desktop"
icon_file="$data_dir/icons/hicolor/scalable/apps/io.github.editshell.EditShell.svg"

removed=0
for file in "$bin_file" "$desktop_file" "$icon_file"; do
    if [[ -e "$file" || -L "$file" ]]; then
        rm -- "$file"
        printf 'Removed %s\n' "$file"
        removed=1
    fi
done

if command -v update-desktop-database >/dev/null 2>&1 \
    && [[ -d "$data_dir/applications" ]]; then
    update-desktop-database "$data_dir/applications" || true
fi

if (( removed == 0 )); then
    printf 'EditShell does not appear to be installed for this user.\n'
else
    printf 'EditShell has been uninstalled for the current user.\n'
fi
