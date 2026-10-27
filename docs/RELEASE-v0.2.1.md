# shadow-ink v0.2.1

v0.2.1 修复 PTY 断开后发送进程残留并持续占用 CPU 的问题；使用能处理 EOF 的终端输入后端，并显式检测终端挂断。QR、逐屏字符和字符流均加入 PTY 关闭回归。打包时以原子替换更新可执行文件，允许运行中的旧程序正常退出。

程序从 `tqr` 更名为 `shadow-ink`，新增默认字符流后端、一条命令获取远程文件和目录，以及自动定向续传。

```bash
shadow-ink server-a /path/to/file.bin
shadow-ink server-a /path/to/file.bin --resume
```

- 自动创建本地 tmux，通过远端终端输出传输 Base64 字符行；默认 `pipe-pane`，可选逐屏 `text` 或兼容 `qr`。
- 保留 CRC32C、RS 64/96、分组 SHA-256 和原文件完整校验。断点在磁盘持久保存，重跑前复核实际数据，只重播缺失或损坏分组。
- 默认一键接收上限为 8 GiB。目录恢复为 tar，传输层 zstd 自动解压。
- 两端需要更新程序名；一键入口要求本机和远端 Python 3.9+，本机另需 tmux 与 SSH。既有 `.tqtx` 二进制协议保持兼容，旧状态和缓存不自动迁移或删除。

发行包包括 Linux x86_64 完整版、Linux x86_64/aarch64 静态发送端，以及 macOS Apple Silicon、Intel、universal2 完整版，附 SHA-256 校验和、许可证及验证记录。

本地不可压缩 64 MiB 测试曾测得约 793 KB/s（原始记录带二进制哈希），这不是实际远程链路或四小时持续稳定性保证。各平台的当前发行二进制和自测结果分别记录，旧版本验证不作为新版本证明。

Linux、Apple Silicon 和 Intel 源码 CI 已通过；六个发行包已校验，Mac 四次原生发行自测全部通过。见 [发布验证](validation/release-v0.2.1.json) 和 [Mac 原生报告](validation/macos-v0.2.1.json)。
