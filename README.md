# shadow-ink

**Bring files home through your terminal.**

Resumable file transfer over terminal output. Base64 by default, QR for visual playback. Automatic tmux setup and SHA-256 verification.

[Download](https://github.com/Chivier/shadow-ink/releases/latest) · [Quick start](#quick-start) · [Documentation](#documentation) · [简体中文](README.zh-CN.md)

```sh
shadow-ink server-a /path/to/results.bin
```

**QR · V12-M**

https://github.com/user-attachments/assets/65f0520c-da18-43f1-9ba9-5782b4842690

**QR · V40-L**

https://github.com/user-attachments/assets/fb685545-d100-4fc0-8ea6-0a02088b7cc6

Two 10-second visualizations with real protocol frames. Playback and image reconstruction are choreographed, not a speed comparison. [HTML source & recording](demo/README.md)

## Quick start

Install `shadow-ink` on **both machines**. The local machine also needs tmux, SSH, and Python 3.9+; the remote machine needs Python 3.9+ and working non-interactive SSH authentication.

```sh
# Download a file. server-a is a hostname or an existing SSH config alias.
shadow-ink server-a /home/user/results.bin

# Download a directory to a chosen local destination.
shadow-ink server-a '/home/user/experiment data' -o ./received
```

Files go to `~/Downloads/from-server-a/` by default. Directories arrive as `.tar` archives. Interrupted transfers resume when you run the same command again; existing files with different content are preserved.

## Install

Get the [latest release](https://github.com/Chivier/shadow-ink/releases/latest), extract the package, and put the executable on your PATH.

| Machine | Package |
| --- | --- |
| macOS receiver or sender | `shadow-ink-*-macos-universal2.tar.gz` — Apple Silicon and Intel |
| Linux receiver | `shadow-ink-*-linux-x86_64.tar.gz` — full build |
| Linux sender | `shadow-ink-send-*-linux-x86_64.tar.gz` or `shadow-ink-send-*-linux-aarch64.tar.gz` — static sender |

Each package includes `SHA256SUMS`. On either machine, install its binary as `shadow-ink`:

```sh
mkdir -p ~/.local/bin
install -m 755 ./shadow-ink ~/.local/bin/shadow-ink
# Ensure ~/.local/bin is on PATH.
shadow-ink --version
```

<details>
<summary>Build from source</summary>

Requires Rust 1.88+, a C/C++ compiler, and CMake. FFmpeg is optional and needed only for video input.

```sh
git clone https://github.com/Chivier/shadow-ink.git
cd shadow-ink
cargo build --release --locked
mkdir -p ~/.local/bin
install -m 755 target/release/shadow-ink ~/.local/bin/shadow-ink

# Smaller sender-only build for the remote machine.
cargo build --release --locked --no-default-features --target-dir target/sender
```

See [build instructions](docs/BUILD.md) and [macOS setup](docs/MACOS.md) for platform details.

</details>

## Choose a transport

| Backend | File data travels as |
| --- | --- |
| `pipe` · default | CRC-protected Base64 lines, read from tmux `pipe-pane` |
| `text` | Complete text lines sampled from a 320×120 tmux canvas |
| `qr` | QR frames sampled from a tmux pane |

```sh
shadow-ink server-a /path/file                       # Base64 stream
shadow-ink server-a /path/file --backend text        # Text sampling
shadow-ink server-a /path/file --backend qr --speed safe
```

All three use the same binary frame protocol, Reed–Solomon recovery, per-frame CRC32C, and SHA-256 checks. The default Base64 path reads terminal output directly, avoiding QR rendering and decoding. [Measured results and their scope](docs/PERFORMANCE.md)

## Resume, or start fresh

```sh
# Continue the saved snapshot, even if the source has since changed.
shadow-ink server-a /path/file --resume

# Take a new snapshot of the current source; keep the old state.
shadow-ink server-a /path/file --fresh
```

The receiver rechecks saved shards and restored groups, then requests missing or damaged groups. Output is published only after the original file's SHA-256 matches. Same-name files with different content are retained; the new file goes into a `transfer-ID/` subdirectory.

Local state lives in `~/.local/state/shadow-ink-get/` (or `$XDG_STATE_HOME/shadow-ink-get/`); remote snapshots live in `~/.cache/shadow-ink-get/`. Keep both to resume. Caches are not automatically deleted.

## How it works

```text
Remote file → snapshot → optional zstd → Reed–Solomon shards
                                             ↓
                                    terminal text or QR
                                             ↓
Local tmux → CRC / deduplication / recovery → SHA-256 → original file
```

The one-command entry point handles SSH, snapshots, tmux, reception, and cleanup of its own sessions. File data travels through the remote terminal output; the lower-level receiver reads local tmux or a local recording.

The one-command size limit is 8 GiB (`--max-bytes` changes it); manual `recv` defaults to 1 GiB. Archives are not automatically extracted. Windows, camera capture, and arbitrary desktop screenshot capture are outside the current scope.

## Documentation

| Guide | Covers |
| --- | --- |
| [One-command transfers](docs/GET.md) | Options, directories, resume, collision handling |
| [Manual transfers & calibration](docs/MANUAL.md) | Existing SSH panes, QR geometry, video input, exit codes |
| [Protocol](docs/PROTOCOL.md) | Frames, manifests, FEC, persistence |
| [Build & platform setup](docs/BUILD.md) | Native builds, static senders, validation |
| [Performance](docs/PERFORMANCE.md) | Completed measurements and their limits |
| [Demo source & recording](demo/README.md) | HTML player and separate minimal MP4 exports |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). For a local check:

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

[Release validation](docs/validation/release-v0.2.1.json) · [Native macOS validation](docs/validation/macos-v0.2.1.json) · [Earlier acceptance record](docs/ACCEPTANCE.md)

## License

[MIT](LICENSE). Bundled font licenses are included with the demo; the reference photograph shown in the videos is not covered by the code's MIT license.
