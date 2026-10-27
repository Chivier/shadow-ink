# Build and deployment

Runtime payload transfer uses terminal text or QR playback. The one-command entry point embeds its Python orchestration helper; Python 3, SSH and tmux are required locally, with Python 3 and a current shadow-ink sender on the remote host. Installing prebuilt tools on A is a separate prerequisite; these instructions do not introduce an alternate outgoing file channel.

## Native builds

Rust 1.88+, C/C++ toolchain, CMake and Perl/pkg-config as required by native library builds. The full build bundles ZXing-C++ and SQLite. Run:

```bash
cargo build --release --locked
cargo build --release --locked --no-default-features --target-dir target/sender
python3 scripts/verify.py
scripts/package.sh
```

`dist/shadow-ink-<version>-<os>-<arch>/shadow-ink` is the full binary; `shadow-ink-send` is the sender-only binary with the same prepare/send/doctor/inspect CLI. The archive includes README, protocol, build instructions, license and checksums. On A, invoke `./shadow-ink-send ...`, or rename it to `shadow-ink` when provisioning.

Native Linux GNU builds require the platform's compatible libc. For isolated Linux server deployments use a musl sender build, targeting x86_64 or aarch64 as appropriate. No receiver/C++ runtime is needed on A.

## Linux static sender cross-builds

With `cargo-zigbuild`, Zig and the Rust targets installed:

```bash
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
cargo zigbuild --locked --release --no-default-features --target x86_64-unknown-linux-musl
cargo zigbuild --locked --release --no-default-features --target aarch64-unknown-linux-musl
python3 scripts/verify_artifacts.py
# Or build and package both server binaries:
scripts/package-static.sh
```

The initial static packages use cargo-zigbuild 0.24.0 and Zig 0.13.0. Pin tool versions in your build environment. Cross-compilation proves the artifact builds; native execution or emulation is a separate validation step, recorded in ACCEPTANCE.md. Runtime sender requires an ANSI/UTF-8 TTY. The local laptop needs tmux for pane capture, FFmpeg for video reception. macOS builds use the same Rust sources but require their own platform build and acceptance run; an unavailable laptop OS/display is not considered tested by Linux results.

## Validation

macOS 提供 Apple Silicon、Intel 和 universal2 完整发行包，最低目标版本为 macOS 11。使用、原生自测和交叉打包说明见 [MACOS.md](MACOS.md)。`scripts/package_macos.py` 需要 `SDKROOT`、cargo-zigbuild、Zig 和 rcodesign；不将 SDK 本身放入发行包。

```bash
cargo fmt --check
cargo test --locked
cargo test --locked --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test acceptance -- --ignored --nocapture --test-threads=1
```

The external acceptance test uses an isolated tmux socket and `/dev/null` configuration, two real panes, and temporary source/output/state paths. It kills only its own tmux server when finished. Test reports are written to `acceptance-output/`.
