# 手动传输与校准

一键下载、目录和自动续传见 [GET.md](GET.md)。本页保留手动 pane、录像输入、终端校准及接收器行为说明。

## 手动字符传输

如果你已有本地 tmux 中的 SSH pane，可在远端运行：

```bash
shadow-ink prepare results.bin --compress auto --symbol-bytes 165 --out results.tqtx
shadow-ink send results.tqtx --backend text --fps 30 --loop
```

在本地另一个 shell 接收：

```bash
shadow-ink panes
shadow-ink recv --source text:%0 --capture-fps 120 --output ./received --state ./rx-state
```

`%0` 替换为本地 SSH pane 的 ID。使用 `--source pipe:%0` 时，远端应改用 `--backend text-stream`；先启动接收器，再启动发送器可避免漏掉开头。自动入口已处理这个顺序。底层 `send` 的默认后端仍是 QR，以便旧手动命令继续工作。

## 手动 QR 兼容流程

先在**笔记本本地**启动 tmux，左右分屏。用 `shadow-ink panes` 查找左侧 pane ID，例如 `%0`。这些命令的 `shadow-ink` 指对应机器上已准备好的二进制。

```bash
# 笔记本 tmux 左侧：进入现有 SSH 终端会话
ssh server-a

# 以下在 A 的 shell 执行。prepare 对输入取一次快照，后续只播放同一个包。
shadow-ink prepare ./results.bin --compress auto --symbol-bytes 224 --out ./results.tqtx
shadow-ink doctor --bundle ./results.tqtx
shadow-ink send ./results.tqtx --qr-version 12 --ecc M --fps 3 --loop
```

```bash
# 笔记本 tmux 右侧，本地 shell
shadow-ink panes
shadow-ink doctor --source tmux:%0
shadow-ink recv --source tmux:%0 --capture-fps 20 --output ./received --state ./rx-state
```

接收端返回 `complete: true` 才表示传输文件和原文件均通过 SHA-256 校验。此后在左侧按 `q` 或 Ctrl-C 停止播放；空格暂停／继续。**A 不会假装知道 B 是否已收完。**

默认循环播放。`--rounds 1` 可用于有限轮次，不能与 `--loop` 同用。收到 SIGINT／SIGTERM 时发送端恢复终端；SIGKILL 和断电无法运行退出清理。

同时接收不同文件时使用不同的 `--state` 目录。一个目录只绑定一个 transfer，且同一时间只允许一个接收器写入。`--transfer-id <32位十六进制ID>` 可以在开始接收前指定允许的传输。

## 断点与定向补传

接收中断后，重新运行同一条 `recv` 命令，保持原 `--state`。A 也可以重启，但必须播放原来的 `.tqtx`。

```bash
shadow-ink status ./rx-state

# status 返回 missing_generations，例如 3,8-10,42。
# 在 A 上播放这些组；分组编号从 0 开始。
shadow-ink send ./results.tqtx --only-generations 3,8-10,42 --fps 3 --loop
```

接收器持久保存未完成组的分片、已完成组及 `.part`，不是只保存百分比。重启时检查 SQLite、缓存分片 CRC 和已恢复组的实际磁盘数据；损坏组重新等待播放。不要删除或移动仍要续传的状态目录。

状态中的统计是**最近一次 recv 运行**的统计；组进度跨运行保存。`observed_forward_sequence_gaps` 是采样时观察到的序号跳跃累计量，不能解释为最终文件丢失量。帧乱序、后续重播和接收器晚启动都不影响按分片身份恢复。

## 录像接收

首版接收现成的本地视频，不负责调用操作系统开始屏幕录制。

```bash
shadow-ink recv --source video:./recording.mp4 --output ./received --state ./rx-state

# 指定像素区域，必须包含完整四模块留白。
shadow-ink recv --source video:./recording.mkv --roi 100,80,600,600 --output ./received --state ./rx-state
```

视频按原始解码顺序逐帧处理，不按实时速度丢旧帧。录像结束仍未收齐时退出码为 2、保留断点；可继续把另一段录像导入同一目录。进度记录视频帧序号与 PTS，便于定位失败位置。支持单帧最大 32MP，网络 URL 和网络协议不属于视频输入接口。

## 终端几何与校准

**120×40 是播放 pane 的可用尺寸，不是左右分屏前整个窗口的尺寸。** tmux 状态栏、边框以及分屏都会减少空间。V12-M 二维码含留白占 73 列×37 行，再加上下标记和状态行；至少需要 74×40 可用单元格。120×40 是建议起点。

| 配置 | 分片 | 完整协议帧 | 播放 pane 建议 |
|---|---:|---:|---|
| V12-M | 224B | 284B / 287B 容量 | 120×40 |
| V17-M | 440B | 500B / 504B 容量 | 160×50 |
| V22-M | 712B | 772B / 779B 容量 | 160×60 |

`.tqtx` 固定分片大小。切到更大的配置需要重新 prepare；那是一个新的传输，使用新的状态目录。调整 fps 或播放窗口不改变传输 ID。

半块字符 `▀ ▄ █ 空格` 每格承载上下两个模块，模块宽高比为 `2 × 字符格像素宽 / 字符格像素高`。字符格宽高接近 1:2 时，模块接近正方形；120 列×40 行本身不能证明字体比例。

```bash
# 不连接真实终端时会如实报告尺寸未知。
shadow-ink doctor
# 可选 300ms、有超时的 CSI 16 t 查询；不自动开启 tmux passthrough。
shadow-ink doctor --probe-pixels
# 已知尺寸时显式提供，不代表已经测量了实际字体。
shadow-ink doctor --cell-pixels 8x16
# 左侧显示固定校准图案，q 退出；它不是一个待接收文件。
shadow-ink doctor --show-pattern
# 右侧观察当前 tmux 采集表现，不修改传输断点。
shadow-ink calibrate --source tmux:%0 --seconds 10 --capture-fps 20
```

tmux 路径根据单元格及颜色还原正方形模块，不依赖字体像素比例。录像路径仍取决于实际字体、行距、缩放和录制质量，应先传一个小文件。`calibrate` 测的是 tmux 字符通道的观测速率，不声称测量了摄像头或实际像素链路。

窗口 resize 后暂停播放；空间恢复后继续。每帧最小保持时间从完成写出后计算，慢终端不会触发补发突发。第一轮参数建议 3fps，稳定后再提高。

## 输出行为

- 自动解压的是传输层 zstd，最终得到 prepare 时的原文件；源文件如果是 `.tar`／`.zip`，不会自动展开归档或执行其中内容。
- 文件名只能是受限 basename。临时文件在输出目录内创建，SHA-256 全部通过后发布；不同内容的同名文件会报错保留，已有相同文件可复核完成。
- 手动 `recv` 的原文件及传输对象大小上限均为 1 GiB；一键入口默认 8 GiB，使用 `--max-bytes` 显式调整。解压限制输出长度，不允许超过 manifest 声明。
- 退出码：`0` 完成／命令成功，`1` 参数或运行错误，`2` 接收未完成／终端或容量检查未通过。进度写 stderr，最终结果写 stdout JSON。
- 程序没有自动清理接收状态；已恢复文件、状态和原传输包均由用户管理。

## 验证与当前范围

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --test acceptance -- --ignored --nocapture --test-threads=1
```

外部验收创建独立 tmux socket，不操作用户的既有会话。它在左侧真实 PTY 播放、右侧本地接收，检查接收中断、发送端重启、resize 暂停及字节一致性；视频验收使用 H.264、丢帧、重复、CRC 错误和多段续传。结果写入 `acceptance-output/*.json`。协议包含独立 Python 实现产生的 RS 测试向量。

协议详见 [docs/PROTOCOL.md](PROTOCOL.md)，验收记录见 [docs/ACCEPTANCE.md](ACCEPTANCE.md)。底层帧协议为 tqr-v1，支持单文件快照、字符或 QR 发送、本机 tmux 和视频接收。qrs-legacy、摄像头、普通终端窗口截图和多码并排是后续扩展，不在本次首版接收范围内。
