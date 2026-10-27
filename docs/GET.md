# 一条命令接收

```bash
shadow-ink server-a /home/user/results.bin
shadow-ink server-a '/home/user/实验结果' -o ./received
```

`server-a` 使用现有 SSH 配置，要求可用的非交互 SSH 认证。本机需要完整 `shadow-ink`、tmux、ssh 和 Python 3.9+；远端需要新版 `shadow-ink` 和 Python 3.9+。远端程序可放在 PATH 或 `~/.local/bin/shadow-ink`。二进制已内嵌编排脚本，安装后无需额外脚本。

工具会自动启动独立的本地 tmux，准备远程固定快照并播放终端字符，从本地 pane 的输出流恢复文件。默认后台运行 tmux，在当前终端输出进度，完成后关闭本次创建的 tmux。底层接收器不连接服务器；SSH 准备步骤只回传元数据，文件通过远程终端输出传输。

默认保存到 `~/Downloads/from-server-a/`。目录保存为 tar，不自动解包。文件在完整 SHA-256 通过后发布；自动解压仅针对传输层 zstd。已有相同文件会独立复核；同名不同内容保留，本次保存到 `transfer-ID/` 子目录。

## 模式与选项

| 选项 | 用途 |
|---|---|
| `--backend pipe` | 默认，从 tmux pipe-pane 连续读取 Base64 行，每行包含一个带 CRC 的协议帧 |
| `--backend text` | 320×120 字符画布，通过 capture-pane 接收完整行；中途重绘不会使整屏作废 |
| `--backend qr` | 兼容 QR 播放与解码 |
| `--fps 30` | 批次/屏刷新上限；pipe 每批 16 个独立包 |
| `--capture-fps 120` | 本地轮询上限，pipe 模式每次会排空一批可用数据 |
| `--timeout 60` | 本次接收限时，保留断点 |
| `--max-bytes 8589934592` | 默认 8 GiB，分别约束原文件和传输对象大小 |
| `--resume` | 显式继续原快照，无本地记录时报错，与 `--fresh` 互斥 |
| `--fresh` | 准备当前内容的新快照，保留旧快照和断点 |
| `-o DIRECTORY` | 本地保存目录 |

Ctrl-C 后，重跑同一命令会继续原快照，即使远端原文件已变化。缺少远端快照时明确报错，使用 `--fresh` 开始新传输。改变分片容量需要 `--fresh`。接收结果、分片和日志保存在 `${XDG_STATE_HOME:-~/.local/state}/shadow-ink-get/`；远端快照位于 `~/.cache/shadow-ink-get/`。两者不自动清理，旧 tqr 缓存继续保留。

## 断点续传

```bash
shadow-ink server-a /path/to/file.bin
# Ctrl-C / 超时 / 断线后，重跑原命令自动续传：
shadow-ink server-a /path/to/file.bin
# 或显式要求继续上次快照：
shadow-ink server-a /path/to/file.bin --resume
```

续传使用相同的服务器、路径和 `-o` 保存目录。保留本地状态目录与远端快照，源文件后来发生变化也继续恢复原快照；`--fresh` 才会重新准备当前内容。显式 `--resume` 对已完成任务也选择原快照并复核已有输出。

接收器先检查 SQLite、缓存分片 CRC 和已恢复分组的实际 SHA-256，再发布本次恢复进度。控制器等校验结束后，用 `--only-generations` 让远端只播放缺失或损坏的分组；未完成组内的有效分片仍然保留。启动时会显示校验后的分组数和已恢复字节数。损坏的 `.part` 分组会重新进入补传范围，避免只按旧数据库计数跳过数据。

没有拿到 manifest 的早期断点会先播放完整序列以恢复元数据；缺失分组参数超过 16 KiB 时会回退为循环播放全部分组，仍复用已有进度。最终必须通过整个文件 SHA-256 才会发布文件。`result.json` 中的 `resume_checkpoint` 和 `replay_generations` 记录这次续传的校验结果与补传范围。

Linux 本地真实 tmux 的续传验证记录见 [验收报告](validation/resume-linux-2026-10-27.json)。CLI 测试以本地命令替代 SSH 网络跳转。

每个请求使用独占锁。自动模式仅关闭自己创建的 pane/server，已有 tmux 会话继续保留。手动 `recv --source pipe:%ID` 遇到已有 pipe 时会拒绝覆盖它。

## QR 档位与测速

```bash
shadow-ink server-a /path/file --backend qr --speed balanced
shadow-ink get --list-speeds
shadow-ink get server-a --backend qr --benchmark
```

QR 保留 safe/balanced/fast/turbo/auto。auto 使用 14 天内与本机二进制匹配、至少两次完整校验的测速推荐；没有有效推荐时使用 safe。QR 接收停滞时默认降低帧率，`--fixed-rate` 可关闭。`--speed` 仅适用于 QR；字符模式用 `--fps`。

本地字符吞吐验证：

```bash
python3 scripts/benchmark-text.py --mib 64 --backend pipe
python3 scripts/benchmark-text.py --mib 2 --backend text --report acceptance-output/text-screen.json
```

这些测试创建隔离的真实 tmux/PTY，使用不可压缩数据并独立复核 SHA-256；它们不证明真实 SSH 带宽或四小时稳定性。

可选的 `scripts/install-get.sh` 仍可安装独立 `shadow-ink-get` 包装脚本和 `kiss-get` 别名；主入口推荐直接使用 `shadow-ink SERVER FILEPATH`。
