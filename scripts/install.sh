#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
binary_source="$project_dir/target/release/editshell"
bin_dir="${HOME:?HOME must be set}/.local/bin"
data_dir="${XDG_DATA_HOME:-"$HOME/.local/share"}"
applications_dir="$data_dir/applications"
icon_dir="$data_dir/icons/hicolor/scalable/apps"
desktop_file="$applications_dir/io.github.editshell.EditShell.desktop"
icon_file="$icon_dir/io.github.editshell.EditShell.svg"
icon_source="$project_dir/assets/io.github.editshell.EditShell.svg"

if ! command -v cargo >/dev/null 2>&1; then
    printf 'Error: Cargo is required to build EditShell.\n' >&2
    exit 1
fi

if ! command -v pkg-config >/dev/null 2>&1 || ! pkg-config --exists gtk4; then
    printf 'Error: GTK 4 development files are required (for example: libgtk-4-dev).\n' >&2
    exit 1
fi

if [[ ! -r "$icon_source" ]]; then
    printf 'Error: application icon not found: %s\n' "$icon_source" >&2
    exit 1
fi

printf 'Building EditShell in release mode...\n'
cargo build --manifest-path "$project_dir/Cargo.toml" --release --features gui

if [[ ! -x "$binary_source" ]]; then
    printf 'Error: expected executable was not produced: %s\n' "$binary_source" >&2
    exit 1
fi

install -Dm755 "$binary_source" "$bin_dir/editshell"
install -Dm644 "$icon_source" "$icon_file"

desktop_exec="$bin_dir/editshell"
desktop_exec="${desktop_exec//\\/\\\\}"
desktop_exec="${desktop_exec//\"/\\\"}"
desktop_exec="${desktop_exec//\`/\\\`}"
desktop_exec="${desktop_exec//\$/\\\$}"
install -d "$applications_dir"
cat >"$desktop_file" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=EditShell
Comment=Interactive Linux shell with command editor
Exec="$desktop_exec"
Icon=io.github.editshell.EditShell
Terminal=false
Categories=System;Utility;TerminalEmulator;
StartupNotify=true
StartupWMClass=io.github.editshell.EditShell
EOF
chmod 644 "$desktop_file"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$applications_dir" || true
fi

printf '\nEditShell installed for the current user.\n'
printf 'Launch it from the applications menu or run: %s/editshell\n' "$bin_dir"
if [[ ":$PATH:" != *":$bin_dir:"* ]]; then
    printf 'Note: %s is not currently in PATH; the desktop launcher still works.\n' "$bin_dir"
fi
