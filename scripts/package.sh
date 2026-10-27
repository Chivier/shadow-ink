#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
python3 scripts/package_ready.py
cargo build --release --locked
cargo build --release --locked --no-default-features --target-dir target/sender
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
name="tqr-$version-$os-$arch"
mkdir -p "dist/$name/docs"
cp target/release/tqr "dist/$name/tqr"
cp target/sender/release/tqr "dist/$name/tqr-send"
cp README.md LICENSE "dist/$name/"
python3 scripts/collect_notices.py "dist/$name/THIRD_PARTY_NOTICES.txt"
cp docs/PROTOCOL.md docs/BUILD.md docs/ACCEPTANCE.md docs/MACOS.md "dist/$name/docs/"
if test -d docs/validation; then cp -R docs/validation "dist/$name/docs/"; fi
mkdir -p "dist/$name/reports"
cp acceptance-output/checks.json acceptance-output/tmux.json acceptance-output/video.json "dist/$name/reports/"
if test -f acceptance-output/artifacts.json; then
    cp acceptance-output/artifacts.json acceptance-output/tmux-static-*.json "dist/$name/reports/"
fi
(cd "dist/$name" && sha256sum tqr tqr-send > SHA256SUMS)
tar -czf "dist/$name.tar.gz" -C dist "$name"
printf '%s\n' "dist/$name.tar.gz"
