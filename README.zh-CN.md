# shadow-ink

**让文件沿着终端回到本机。**

默认通过 Base64 终端输出接收文件，也支持二维码播放。自动启动 tmux、保存断点，并校验 SHA-256。

[下载](https://github.com/Chivier/shadow-ink/releases/latest) · [快速开始](#快速开始) · [文档](#文档) · [English](README.md)

```sh
shadow-ink server-a /path/to/results.bin
```

**QR · V12-M**

https://github.com/user-attachments/assets/65f0520c-da18-43f1-9ba9-5782b4842690

**QR · V40-L**

https://github.com/user-attachments/assets/fb685545-d100-4fc0-8ea6-0a02088b7cc6

两段各 10 秒，使用真实协议帧；播放节奏和图像还原为演示编排，不代表速度对比。[HTML 源码与录制说明](demo/README.md)

## 快速开始

**两端都需要安装 `shadow-ink`。** 本机另需 tmux、SSH 和 Python 3.9+；远端需要 Python 3.9+，并可通过现有 SSH 配置免交互登录。

```sh
# server-a 是主机名或 ~/.ssh/config 中的别名。
shadow-ink server-a /home/user/results.bin

# 下载目录，并指定本机保存位置。
shadow-ink server-a '/home/user/实验数据' -o ./received
```

默认保存到 `~/Downloads/from-server-a/`。目录以 `.tar` 归档保存；中断后重跑原命令即可续传。已有同名、不同内容的文件会保留，新文件放入 `transfer-ID/` 子目录。

## 安装

下载[最新发行包](https://github.com/Chivier/shadow-ink/releases/latest)，解压后把程序放入 PATH。

| 安装位置 | 发行包 |
| --- | --- |
| macOS 接收端或发送端 | `shadow-ink-*-macos-universal2.tar.gz`，兼容 Apple Silicon 与 Intel |
| Linux 接收端 | `shadow-ink-*-linux-x86_64.tar.gz`，完整版本 |
| Linux 发送端 | `shadow-ink-send-*-linux-x86_64.tar.gz` 或 `shadow-ink-send-*-linux-aarch64.tar.gz`，静态版本 |

发行包附带 `SHA256SUMS`。两端都将包内程序安装为 `shadow-ink`：

```sh
mkdir -p ~/.local/bin
install -m 755 ./shadow-ink ~/.local/bin/shadow-ink
# 确保 ~/.local/bin 位于 PATH 中。
shadow-ink --version
```

<details>
<summary>从源码构建</summary>

需要 Rust 1.88+、C/C++ 编译器和 CMake。录像接收另需 FFmpeg。

```sh
git clone https://github.com/Chivier/shadow-ink.git
cd shadow-ink
cargo build --release --locked
mkdir -p ~/.local/bin
install -m 755 target/release/shadow-ink ~/.local/bin/shadow-ink

# 远端可单独构建较小的发送版本。
cargo build --release --locked --no-default-features --target-dir target/sender
```

平台要求见[构建说明](docs/BUILD.md)和 [macOS 说明](docs/MACOS.md)。

</details>

## 传输方式

| 后端 | 文件数据经过的通道 |
| --- | --- |
| `pipe`，默认 | tmux `pipe-pane` 读取带 CRC 的 Base64 行 |
| `text` | 从 320×120 的 tmux 画布采集完整字符行 |
| `qr` | 从 tmux pane 采集二维码帧 |

```sh
shadow-ink server-a /path/file                       # Base64 字符流
shadow-ink server-a /path/file --backend text        # 逐屏字符采样
shadow-ink server-a /path/file --backend qr --speed safe
```

三种方式使用相同的二进制帧协议、Reed–Solomon 恢复、逐帧 CRC32C 和 SHA-256 校验。默认 Base64 模式直接读取终端输出，省去二维码绘制和解码。[性能记录与测量边界](docs/PERFORMANCE.md)

## 续传与新快照

```sh
# 继续上次快照，即使远端源文件已经变化。
shadow-ink server-a /path/file --resume

# 获取当前内容的新快照，保留旧快照和断点。
shadow-ink server-a /path/file --fresh
```

接收器先复核已保存的分片和已恢复分组，再补传缺失或损坏的分组。原文件 SHA-256 通过后才发布输出。

本机状态保存在 `~/.local/state/shadow-ink-get/`，也可由 `$XDG_STATE_HOME` 指定；远端快照在 `~/.cache/shadow-ink-get/`。两者都要保留才能续传，程序不会自动清理缓存。

## 工作流程

```text
远端文件 → 固定快照 → 可选 zstd → Reed–Solomon 分片
                                      ↓
                                终端字符或二维码
                                      ↓
本地 tmux → CRC / 去重 / 恢复 → SHA-256 → 原文件
```

一键入口负责 SSH 编排、快照、本地 tmux 和接收，只清理自己创建的会话。文件数据经远端终端输出传输；底层接收器只读取本地 tmux 或本地录像。

一键入口默认上限为 8 GiB，手动 `recv` 默认 1 GiB，均可用 `--max-bytes` 调整。归档不会自动解包。Windows、摄像头和任意桌面窗口截图不在当前支持范围内。

## 文档

| 文档 | 内容 |
| --- | --- |
| [一键接收](docs/GET.md) | 选项、目录、续传、同名保护 |
| [手动传输与校准](docs/MANUAL.md) | 现有 SSH pane、QR 几何、录像输入、退出码 |
| [协议](docs/PROTOCOL.md) | 帧、manifest、纠错与持久化 |
| [构建与部署](docs/BUILD.md) | 原生构建、静态发送端、验证 |
| [性能记录](docs/PERFORMANCE.md) | 完整测量结果与边界 |
| [演示与录制](demo/README.md) | HTML 播放器和独立 MP4 导出 |

## 参与开发

见 [CONTRIBUTING.md](CONTRIBUTING.md)。本地检查：

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

[发行验证](docs/validation/release-v0.2.1.json) · [macOS 原生验收](docs/validation/macos-v0.2.1.json) · [历史验收](docs/ACCEPTANCE.md)

## 许可证

[MIT](LICENSE)。演示字体附带各自的许可证；视频中的参考照片不属于代码的 MIT 授权范围。
