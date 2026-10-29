# CLI reference

Help captured from `shadow-ink 0.2.1` with trailing padding trimmed. Refresh with `python3 scripts/update-cli-reference.py`.

## get

```text
usage: shadow-ink get [-h] [--backend {pipe,text,qr}] [-o OUTPUT]
                      [--fresh | --resume]
                      [--speed {auto,safe,balanced,fast,turbo}]
                      [--list-speeds] [--benchmark] [--fps FPS]
                      [--capture-fps CAPTURE_FPS] [--fixed-rate]
                      [--timeout TIMEOUT] [--max-bytes MAX_BYTES] [--no-ui]
                      [source] [remote_path]

自动启动本地 tmux，通过 SSH 终端字符接收远程文件或文件夹。用法：shadow-ink SERVER FILEPATH

positional arguments:
  source                SSH 服务器；也支持服务器:远程路径
  remote_path           远程文件路径；也支持 SERVER:PATH 写法

options:
  -h, --help            show this help message and exit
  --backend {pipe,text,qr}
                        pipe（默认）：tmux 输出流；text：逐行字符采样；qr：兼容二维码
  -o OUTPUT, --output OUTPUT
                        保存目录，默认 ~/Downloads/from-服务器
  --fresh               重新准备当前内容，不续传旧快照
  --resume              继续上次快照；没有本地断点时明确报错（默认自动续传未完成任务）
  --speed {auto,safe,balanced,fast,turbo}
                        auto（默认）使用有效实测推荐；其余为速度档位
  --list-speeds         显示速度档位
  --benchmark           实测二维码容量 × 10/20/30/60 fps，并保存自动档位
  --fps FPS             覆盖档位发送帧率，0.1–60
  --capture-fps CAPTURE_FPS
                        覆盖档位接收采样率，0.1–120
  --fixed-rate          固定发送帧率，停滞时不自动降帧
  --timeout TIMEOUT     接收时限（秒），到时保存断点
  --max-bytes MAX_BYTES
                        最大文件字节数，默认 8 GiB
  --no-ui               后台传输，前台仅显示进度
```

## prepare

```text
Snapshot and optionally compress a file into an immutable playback bundle

Usage: shadow-ink prepare [OPTIONS] --out <OUT> <INPUT>

Arguments:
  <INPUT>

Options:
      --out <OUT>
      --compress <COMPRESS>          [default: auto] [possible values: none, auto, zstd]
      --symbol-bytes <SYMBOL_BYTES>  [default: 224]
      --fec <FEC>                    [default: rs:64:96]
  -h, --help                         Print help
```

## send

```text
Play text or QR frames in the current terminal. q quits; space pauses

Usage: shadow-ink send [OPTIONS] <BUNDLE>

Arguments:
  <BUNDLE>

Options:
      --backend <BACKEND>                    [default: qr] [possible values: qr, text, text-stream]
      --qr-version <QR_VERSION>              [default: 12]
      --ecc <ECC>                            [default: M] [possible values: L, M, Q, H]
      --fps <FPS>                            [default: 3]
      --loop
      --rounds <ROUNDS>
      --only-generations <ONLY_GENERATIONS>
  -h, --help                                 Print help
```

## recv

```text
Recover from a local tmux pane or a recorded video. Never connects to server A

Usage: shadow-ink recv [OPTIONS] --source <SOURCE> --output <OUTPUT> --state <STATE>

Options:
      --source <SOURCE>
      --output <OUTPUT>
      --state <STATE>
      --tmux-socket <TMUX_SOCKET>
      --capture-fps <CAPTURE_FPS>  [default: 20]
      --timeout <TIMEOUT>
      --max-bytes <MAX_BYTES>      [default: 1073741824]
      --transfer-id <TRANSFER_ID>
      --roi <ROI>
  -h, --help                       Print help
```

## status

```text
Show persisted progress, missing generation ranges and reception statistics

Usage: shadow-ink status <STATE>

Arguments:
  <STATE>

Options:
  -h, --help  Print help
```

## inspect

```text
Validate a prepared bundle and show its manifest and transfer identity

Usage: shadow-ink inspect <BUNDLE>

Arguments:
  <BUNDLE>

Options:
  -h, --help  Print help
```

## doctor

```text
Check terminal geometry, pixel aspect and QR capacity; optionally show a pattern

Usage: shadow-ink doctor [OPTIONS]

Options:
      --source <SOURCE>
      --tmux-socket <TMUX_SOCKET>
      --bundle <BUNDLE>
      --qr-version <QR_VERSION>    [default: 12]
      --ecc <ECC>                  [default: M] [possible values: L, M, Q, H]
      --cell-pixels <CELL_PIXELS>
      --probe-pixels
      --show-pattern
  -h, --help                       Print help
```

## panes

```text
List local tmux pane IDs and usable dimensions

Usage: shadow-ink panes [OPTIONS]

Options:
      --tmux-socket <TMUX_SOCKET>
  -h, --help                       Print help
```

## calibrate

```text
Measure local tmux QR observations while the sender plays in another pane

Usage: shadow-ink calibrate [OPTIONS] --source <SOURCE>

Options:
      --source <SOURCE>
      --tmux-socket <TMUX_SOCKET>
      --seconds <SECONDS>          [default: 10]
      --capture-fps <CAPTURE_FPS>  [default: 20]
  -h, --help                       Print help
```
