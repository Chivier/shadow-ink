#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
destination=${TQR_GET_BIN_DIR:-"$HOME/.local/bin"}
mkdir -p "$destination"
if [ -e "$destination/kiss-get" ] || [ -L "$destination/kiss-get" ]; then
    if [ ! -L "$destination/kiss-get" ] || [ "$(readlink "$destination/kiss-get")" != tqr-get ]; then
        printf '%s\n' "Refusing to replace existing $destination/kiss-get" >&2
        exit 1
    fi
fi
install -m 755 scripts/tqr-get "$destination/tqr-get"
if [ ! -L "$destination/kiss-get" ]; then
    ln -s tqr-get "$destination/kiss-get"
fi
printf '%s\n' "Installed $destination/tqr-get and $destination/kiss-get"
