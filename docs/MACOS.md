# macOS

新版命令名为 `shadow-ink`，一键接收需要 macOS 本机 tmux、SSH 和 Python 3.9+；远端需要新版发送程序和 Python 3.9+。

```bash
brew install rust tmux cmake python
cargo build --release --locked
mkdir -p ~/.local/bin
install -m 755 target/release/shadow-ink ~/.local/bin/shadow-ink
shadow-ink server-a /path/to/results.bin
```

macOS 从源码原生构建时，Rust 工具链会为当前机器生成 Apple Silicon 或 Intel 二进制。`scripts/package_macos.py` 可构建 thin/universal2 发行包，使用新程序名，保留签名、校验和及原生 CI 自测流程。

下载 [v0.2.1 发行包](https://github.com/Chivier/shadow-ink/releases/tag/v0.2.1) 时，优先选择 `shadow-ink-0.2.1-macos-universal2.tar.gz`；它包含 Apple Silicon 和 Intel 两种架构。解压后可运行：

```bash
codesign --verify --strict --all-architectures ./shadow-ink
python3 selftest.py
./shadow-ink server-a /path/to/results.bin
```

发行包包含 SHA-256 校验和及 `selftest.py` / `cli_selftest.py`。自测会校验包内程序和脚本，验证 QR、zstd、字符采样、字符输出流、中断续传及损坏分组定向补传；CLI 的网络跳转使用本地替身，真实 tmux/PTY 和实际发行二进制参与执行。

本地 Linux 交叉构建只证明 Mach-O 架构、依赖和签名页面哈希。原生 Apple `codesign` 和运行验收由 GitHub Actions 在 Apple Silicon 与 Intel 上分别执行 thin/universal2 包，记录具体二进制 SHA-256。历史 v0.1.0 验证保留在 `docs/validation/macos-v0.1.0.json`；每个版本的原生记录独立保存。

录像接收额外需要 `brew install ffmpeg`。使用隔离或签名策略时，按你自己的部署流程验证下载的二进制；重新构建的程序哈希会不同于历史发行包。

v0.2.1 已在 Apple Silicon 和 Intel 上分别完成 thin/universal2 发行二进制原生自测，包括字符传输和定向续传。完整哈希及结果见 [原生验收记录](validation/macos-v0.2.1.json)。
