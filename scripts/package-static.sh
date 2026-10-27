#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo zigbuild --release --locked --no-default-features --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl
python3 scripts/package_ready.py static
mkdir -p dist
python3 scripts/collect_notices.py dist/THIRD_PARTY_NOTICES.txt
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
for target in x86_64 aarch64; do
    name="tqr-send-$version-linux-$target"
    mkdir -p "dist/$name/docs"
    cp "target/$target-unknown-linux-musl/release/tqr" "dist/$name/tqr"
    cp README.md LICENSE dist/THIRD_PARTY_NOTICES.txt "dist/$name/"
    cp docs/PROTOCOL.md docs/BUILD.md docs/ACCEPTANCE.md "dist/$name/docs/"
    mkdir -p "dist/$name/reports"
    cp acceptance-output/checks.json acceptance-output/artifacts.json acceptance-output/tmux*.json acceptance-output/video.json "dist/$name/reports/"
    (cd "dist/$name" && sha256sum tqr > SHA256SUMS)
    tar -czf "dist/$name.tar.gz" -C dist "$name"
    printf '%s\n' "dist/$name.tar.gz"
done
