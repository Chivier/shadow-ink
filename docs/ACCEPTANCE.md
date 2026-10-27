# tqr 0.1.0 acceptance record

Validated on 2026-10-27, Linux x86_64, Rust 1.91.1, tmux 3.4, FFmpeg 6.1.1, OpenSSH 9.6p1. The accepted scope is the agreed first release: a single immutable file bundle, TUI QR playback on A, and local tmux/video reception on the laptop. Camera, desktop screenshot and qrs-legacy adapters remain outside this release.

## Gates

All seven gates in `scripts/verify.py` passed. `acceptance-output/checks.json` records commands, exit codes, durations and the source/test/vector fingerprint:

```text
fcda500ac0aae394490b5f02689720ef115c89001cf55dcce0a253f376f44429
```

| Gate | Result |
|---|---|
| Rust formatting | Passed |
| Default-feature suite | 22 tests passed; the two external tests are run separately below |
| Sender-only suite | 12 tests passed |
| Clippy, all targets/features, warnings denied | Passed |
| Clippy, sender-only, warnings denied | Passed |
| Optimized full release build | Passed |
| External acceptance using the optimized release binary | Both tests passed |

Regular tests cover frame endian/layout and CRC corruption, malformed lengths, independent Python frame/RS vectors, random and burst losses, source/package tampering, empty/Unicode/binary/zstd inputs, duplicate/conflicting symbols, late metadata, state binding, corrupted shards/part ranges, concurrent writer exclusion, unknown-state preservation and output collision/tampering recovery. A 1MiB file crosses 74 generation windows with deterministic 20% frame erasures and bounded cached symbols, then matches source bytes exactly.

## Real tmux and video

`acceptance-output/tmux.json`: an isolated tmux server with two real panes and PTYs. Left plays the TUI, right runs the local receiver. The test interrupts and resumes reception, SIGKILLs a receiver, restarts playback with a new run ID, and shrinks/restores the window. The public panes/status/calibrate commands, pause/resume/quit keys, alternate-screen restoration, calibration pattern and bounded pixel-size probe are exercised in the same PTYs. Two groups / 20,000 original bytes are recovered and compared byte-for-byte. SHA-256:

```text
fd8627e6051ea659e3fa9efc192c0bd8c3e1591bf63aa79f3a79a13184257da0
```

`acceptance-output/video.json`: two H.264 recordings with deterministic 20% data-frame erasures, duplicate images, a QR containing a bad protocol CRC and a foreign transfer. The first recording ends incomplete with persisted shards; the second completes the same state directory. ROI and decoded video PTS are checked. The 17,000-byte output matches the source exactly. SHA-256:

```text
9b9db55c8c3e4a58c51503053138774800f9cdb46d22a712f47c63b7cd82f2a6
```

Video frames in this test are rendered from the actual encoded QR matrix and H.264 compressed. This verifies the image/video path, not the appearance of an unavailable laptop font.

## Static server artifacts

Both server binaries are stripped, statically linked musl ELF files. Builds use cargo-zigbuild 0.24.0 and Zig 0.13.0. `scripts/verify_artifacts.py` verifies ELF architecture/no dynamic interpreter, `help`, Zstandard preparation and native inspection; it then runs the two-pane restart/crash/resize test against each artifact:

| Artifact | Runtime acceptance |
|---|---|
| Linux x86_64 static sender | **Real SSH TUI channel**, native execution |
| Linux aarch64 static sender | QEMU aarch64 8.2.2, actual PTY/TUI execution |

The SSH test launches a temporary, unprivileged server listening only on loopback, with ephemeral keys and no existing user SSH configuration. A ForceCommand accepts only this sender's `send` command; no SFTP subsystem or forwarding is enabled. The left pane runs `ssh -tt` and receives only TUI playback. The right local receiver reconstructs the file from that pane, surviving receiver SIGKILL, sender restart and terminal resize. No SCP or alternate outgoing file transport is used.

Machine-readable results and binary SHA-256 values are in `acceptance-output/artifacts.json`, `tmux-static-x86-ssh.json` and `tmux-static-arm-qemu.json`. The release archives include these reports and their own binary checksum files.

## Limits of the evidence

The user's actual server A, Mac, font and screen recording were not supplied for direct access. The tests prove the software and the stated Linux/SSH/tmux/video paths in this environment; they do not claim deployment to that host, physical aarch64 hardware testing, macOS/Windows acceptance, or a measured laptop character-cell ratio. The tmux path reconstructs logical cells and does not require a 1:2 font. For video on the laptop, start at the documented 3fps and verify a small file before increasing speed.

Run `python3 scripts/verify.py` to repeat the release gates. With Zig, cargo-zigbuild, OpenSSH server/keygen and an ARM emulator available, repeat static artifact acceptance with `python3 scripts/verify_artifacts.py`. Use `TQR_ARM_EMULATOR` to point at an installed emulator.

## macOS distribution

Complete aarch64, x86_64 and universal2 receivers are built for macOS 11+. Cross-build inspection checks architecture, system-only dynamic dependencies and each SHA-256 CodeDirectory page. The private repository runs native macOS source tests on Apple Silicon and Intel, and release events download the exact published binaries for Apple codesign verification and local TUI/zstd/resume selftests. See the release-linked Actions results and native JSON artifacts for the current runtime verdict; build reports do not substitute for native execution.
