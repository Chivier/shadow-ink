<h1 align="center">shadow-ink</h1>

<p align="center">Resumable file transfer over terminal output.</p>

<p align="center">
  <a href="https://github.com/Chivier/shadow-ink/actions/workflows/check.yml"><img src="https://github.com/Chivier/shadow-ink/actions/workflows/check.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/Chivier/shadow-ink/releases/latest"><img src="https://img.shields.io/github/v/release/Chivier/shadow-ink" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/Chivier/shadow-ink" alt="MIT license"></a>
</p>

<p align="center">
  <a href="https://chivier.github.io/shadow-ink/">Documentation</a> ·
  <a href="#installation">Installation</a> ·
  <a href="#usage">Usage</a> ·
  <a href="README.zh-CN.md">简体中文</a>
</p>

shadow-ink brings remote files to your machine through Base64 text or QR playback. It handles SSH, tmux, checkpoints and SHA-256 verification in one command.

```sh
shadow-ink server-a /path/to/results.bin
```

## Demo

**QR · V12-M**

https://github.com/user-attachments/assets/65f0520c-da18-43f1-9ba9-5782b4842690

**Base64 · pipe**

https://github.com/user-attachments/assets/bbfda50e-38a6-4ee9-8366-99c880bc5c2e

<sub>Real protocol frames; choreographed playback and image reveal. These clips are not throughput benchmarks. [Recording source](demo/README.md).</sub>

## Features

- **One command.** Prepares a remote snapshot and starts its own local tmux session.
- **Resumable.** Rechecks saved shards and requests missing or damaged groups.
- **Verified output.** CRC32C, Reed–Solomon recovery and complete-file SHA-256 checks.
- **Three transports.** Base64 stream, sampled text and QR, with a shared frame protocol.
- **Files and directories.** Directory archives and same-name file protection on macOS and Linux.

## Installation

Download the [latest release](https://github.com/Chivier/shadow-ink/releases/latest) for each machine:

| Machine | Package |
| --- | --- |
| macOS | `shadow-ink-*-macos-universal2.tar.gz` — Apple Silicon and Intel |
| Linux receiver | `shadow-ink-*-linux-x86_64.tar.gz` |
| Linux sender | `shadow-ink-send-*-linux-{x86_64,aarch64}.tar.gz` — static build |

Verify the included `SHA256SUMS`, extract the package, then install its executable as `shadow-ink`:

```sh
mkdir -p ~/.local/bin
install -m 755 ./shadow-ink ~/.local/bin/shadow-ink
# Ensure ~/.local/bin is on PATH.
shadow-ink --version
```

Both machines need `shadow-ink` and Python 3.9+. The receiver also needs tmux and SSH; the remote host must support non-interactive SSH authentication.

[Build from source](docs/BUILD.md) · [macOS setup](docs/MACOS.md)

## Usage

Use any hostname or alias from your SSH configuration:

```sh
# Download a file with the default Base64 stream.
shadow-ink server-a /home/user/results.bin

# Download a directory to a chosen local destination.
shadow-ink server-a '/home/user/experiment data' -o ./received

# Continue the saved snapshot, or take a new one.
shadow-ink server-a /path/file --resume
shadow-ink server-a /path/file --fresh

# Choose visual QR playback or sampled text.
shadow-ink server-a /path/file --backend qr --speed safe
shadow-ink server-a /path/file --backend text
```

Downloads default to `~/Downloads/from-server-a/`. Directories arrive as `.tar` archives. Interrupted transfers resume when you repeat the same command. Existing files with different content are preserved.

`--resume` keeps the original snapshot even if the source changes; `--fresh` creates a new one. Keep the local checkpoint and remote snapshot to resume. The one-command size limit defaults to 8 GiB.

[All options and examples →](https://chivier.github.io/shadow-ink/#parameters)

## Documentation

- [Getting started](https://chivier.github.io/shadow-ink/#quick-start)
- [Backend selection](https://chivier.github.io/shadow-ink/#transports)
- [Resume and snapshots](https://chivier.github.io/shadow-ink/#resume)
- [Parameter reference](https://chivier.github.io/shadow-ink/#parameters)
- [Manual reception and video input](docs/MANUAL.md)
- [Protocol](docs/PROTOCOL.md) · [Performance measurements](docs/PERFORMANCE.md)

The documentation site is maintained in [`docs/`](docs/README.md).

## Contributing

Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and validation. Include your version, OS, backend and a small reproducible example when reporting a problem.

## License

[MIT](LICENSE). Demo fonts retain their bundled licenses. The reference photograph shown in the clips is separate from the code's MIT license.
