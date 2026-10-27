#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
destination=${SHADOW_INK_BIN_DIR:-${TQR_GET_BIN_DIR:-"$HOME/.local/bin"}}
mkdir -p "$destination"
if [ -e "$destination/kiss-get" ] || [ -L "$destination/kiss-get" ]; then
    if [ ! -L "$destination/kiss-get" ] || { [ "$(readlink "$destination/kiss-get")" != shadow-ink-get ] && [ "$(readlink "$destination/kiss-get")" != tqr-get ]; }; then
        printf '%s\n' "Refusing to replace existing $destination/kiss-get" >&2
        exit 1
    fi
fi
install -m 755 scripts/shadow-ink-get "$destination/shadow-ink-get"
ln -sfn shadow-ink-get "$destination/kiss-get"
printf '%s\n' "Installed $destination/shadow-ink-get and $destination/kiss-get"
