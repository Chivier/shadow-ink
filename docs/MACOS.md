# 在 Mac 上接收

提供 macOS 11+ 的三个完整版本：

- **universal2：推荐**，同一个 `tqr` 含 Apple Silicon 和 Intel 两种架构。
- aarch64：Apple Silicon（M 系列）。
- x86_64：Intel Mac。

接收、发送、准备文件、断点状态和校准命令均包含在内。二维码识别、SQLite、Zstandard 及 C++ 标准库已链接进程序，动态依赖只使用 macOS 自带系统库和 Foundation；不需要安装 ZXing、SQLite 或 Rust。

交叉构建会检查 Mach-O 架构、最低系统版本、动态依赖和每个签名代码页的 SHA-256。macOS 原生运行结果另由私有仓库的 Apple Silicon／Intel CI 记录，包括 Apple `codesign` 验证及下载的实际发行包自测；请以发行页关联的 CI 和原生报告为准，不将 Linux 测试标记为 macOS 运行验收。发行包内也提供本机自测脚本。

**v0.1.0 已通过 macOS 15.7.9 的 Apple Silicon 和 Intel 原生发行包验收**：两种机器分别验证 universal2 和对应单架构包，检查 Apple 签名、压缩、TUI 接收、重启续传及原文件逐字节一致性。[原生验收任务](https://github.com/Chivier/shadow-ink/actions/runs/40434697444)，机器报告见包内 `reports/macos-native.json`。

## 安装与首次检查

下载 `tqr-0.1.0-macos-universal2.tar.gz` 并解压，然后进入该目录：

```bash
shasum -a 256 -c SHA256SUMS
codesign --verify --strict --all-architectures ./tqr
./tqr --help
```

这是 ad-hoc 签名，未使用 Apple Developer ID，也未做 Apple 公证。如果 macOS 阻止浏览器下载的这个程序，先核对校验和和来源，再仅对当前目录中的这个文件执行：

```bash
xattr -d com.apple.quarantine ./tqr
```

不需要关闭系统 Gatekeeper 或修改全局安全设置。ad-hoc 签名用于代码完整性，不代表 Apple 已确认发布者身份。

如已安装 Homebrew：

```bash
brew install tmux
# 只有接收录像时才需要：
brew install ffmpeg
```

`tqr` 本身不需要 Python；包内自测脚本使用 Python 3。可将二进制放入用户自己的 PATH 目录，也可一直使用 `./tqr`。

## 你的左右分屏流程

在 **Mac 本地**启动 tmux 并左右分屏。左侧 SSH 到 A，右侧保留 Mac 的本地 shell。tmux 直接读取字符和颜色，不需要屏幕截图权限。

左侧 A：

```bash
# 使用 A 对应的 Linux 发送端，沿用原来的 tqr-v1 协议。
tqr prepare ./results.bin --compress auto --symbol-bytes 224 --out ./results.tqtx
tqr send ./results.tqtx --qr-version 12 --ecc M --fps 3 --loop
```

右侧 Mac：

```bash
./tqr panes
# 将 %0 换成左侧 pane 的实际 ID。
./tqr doctor --source tmux:%0
./tqr recv --source tmux:%0 --output ./received --state ./rx-state
```

播放 pane 建议至少 120×40。中断后用相同 `--state` 继续；A 仍播放原 `.tqtx`。接收结果 `complete: true` 才代表原文件通过校验；传输层 zstd 会自动解压。

录像接收同样在 Mac 本地运行：

```bash
./tqr recv --source video:./recording.mp4 --output ./received --state ./video-state
```

## Mac 本机自测

```bash
python3 selftest.py --report ./macos-selftest-result.json
```

脚本校验发行文件和签名，创建独立的临时 tmux 会话，进行压缩、二维码播放、部分接收、发送器重启、磁盘续传和原文件逐字节比对。不会操作你已有的 tmux 会话或连接远程服务器。它检测的是本机字符通道，不是字体像素或录像画面。生成的报告明确记录本机系统、架构、二进制哈希及自测结果。

## 从源码构建与复验

Mac 安装 Xcode Command Line Tools、Rust 1.88+、CMake 后，可原生构建：

```bash
cargo build --release --locked
cargo test --locked
cargo test --locked --test acceptance -- --ignored --nocapture --test-threads=1
```

项目另包含 macOS CI 任务；CI 配置的存在不表示这些任务已在本次运行中执行。

交叉发行使用 `scripts/package_macos.py`：需要 cargo-zigbuild、Zig、macOS SDK（`SDKROOT`）和 rcodesign（或 `TQR_RCODESIGN` 指定路径）。它构建 universal2 和两个 thin 版本，再以空签名配置、无网络时间戳的 ad-hoc 模式签名。`acceptance-output/macos.json` 和包内报告保存二进制检查结果，原 Linux 验收报告明确标记为 Linux。

参考：[Homebrew tmux](https://formulae.brew.sh/formula/tmux)、[Homebrew FFmpeg](https://formulae.brew.sh/formula/ffmpeg)、[rcodesign 签名说明](https://gregoryszorc.com/docs/apple-codesign/stable/apple_codesign_rcodesign_signing.html)、[Apple 分发签名说明](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/)。
