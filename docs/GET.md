# 一条命令接收

在 **Mac 本地终端**运行：

```bash
kiss-get /home/chivier/results.csv
```

`kiss-get` 默认从 `wgkissshot` 获取数据。其他服务器使用：

```bash
tqr-get server-a:/home/user/results.csv
```

文件默认保存在 `~/Downloads/from-wgkissshot/`，其他服务器按 SSH 名称建立保存目录。完成后显示文件位置和 SHA-256 校验结果。路径含空格时给整个参数加引号：

```bash
kiss-get '/home/chivier/实验结果/results.csv'
```

也可以使用远程用户主目录下的相对路径：`kiss-get results.csv`。远程 `~` 需要引号，避免在 Mac 上展开：`kiss-get '~/results.csv'`。

## 自动完成的操作

工具通过现有 SSH 配置连接服务器，准备不可变传输包，启动二维码播放，在 Mac 上接收并独立复核 SHA-256，最后停止发送。文件内容经 SSH 终端二维码传输，接收器仍只读取本地 tmux pane；SSH 准备连接只返回文件名、哈希和传输标识等元数据。

稳妥档在尺寸足够的现有 tmux pane 内运行时，在左侧新增发送 pane，当前 pane 显示进度，完成后关闭新增 pane。普通终端尺寸足够时，自动进入二维码和进度分屏，完成后返回。较大二维码的档位默认使用独立后台 tmux 画布，前台显示进度，减少可见终端布局对采集的干扰。小窗口或非交互运行同样使用后台画布。不会关闭已有 tmux 会话。

## 速度档位

```bash
kiss-get /home/chivier/results.csv --speed balanced
kiss-get /home/chivier/results.csv --speed fast
kiss-get --list-speeds
```

| 档位 | 二维码 | 数据分片 | 发送上限 | 接收采样上限 |
|---|---|---:|---:|---:|
| `safe` 稳妥 | V12-M | 224 B | 10 fps | 40 fps |
| `balanced` 均衡 | V22-M | 712 B | 20 fps | 80 fps |
| `fast` 快速 | V40-L | 2893 B | 30 fps | 120 fps |
| `turbo` 极限 | V40-L | 2893 B | 60 fps | 120 fps |
| `auto` 默认 | 使用本机对该服务器的有效测速推荐；无有效推荐时用 `safe` | | | |

这些是配置参数，不是实测帧率或速度保证。发送还包含编码和输出耗时；采样还包含 tmux 捕获、二维码解码及状态写入。极限档位可能比快速档位慢。

日常传输如果连续 5 秒没有新增分片，会自动降低发送帧率并重播原快照，保留接收端已收内容，最低降至 3 fps。实际使用过的帧率和降帧事件写入传输记录。需要固定参数做实验时加 `--fixed-rate`；一键测速始终固定每个候选的参数，不启用自动降帧。

V12、V22、V40 的播放 pane 分别至少需要 74×40、114×60、186×96 个字符单元格。小终端也可以使用大二维码，工具会自动创建足够大的后台画布。tmux 读取的是字符和颜色状态，见 [capture-pane 手册](https://man.openbsd.org/tmux#capture-pane)。

### 一键测速与自动选择

```bash
kiss-get --benchmark
# 其他服务器：
tqr-get server-a --benchmark
```

测速自动生成不易压缩的临时样本，通过真实 SSH 终端二维码发送，比较 V12-M、V22-M、V40-L × 10/20/30/60 fps，同时测量原 3 fps 配置和预设档位。每个样本包含两个 RS 分组，只有完整恢复并独立通过 SHA-256 校验才算成功。

报告分别记录配置发送帧率、依据帧序号推算的实际发送帧率、实际采样率、每秒新增数据分片，以及经过完整校验的有效字节速度。有效速度包含 SSH 播放连接启动、接收、最终校验和发送端收尾的时间；准备和压缩时间单独排除。不同数据帧数不包含重复帧和元数据帧。

最快的两个候选各再测一次，按两次中较慢的有效速度选择 `auto` 配置。推荐与 Mac 接收程序 SHA-256 绑定，有效期 14 天。更新接收程序或推荐过期后回到 `safe`；网络或服务器负载变化时可重新测速。短样本结果不代表多小时传输的稳定性。

参见 [2026-10-27 Mac 与 wgkissshot 的实测结果](PERFORMANCE.md)。

报告和推荐保存在本机状态目录的 `benchmarks/服务器/` 下。源码优化合并了每次采样的三个 tmux 子进程，保留前后尺寸检查；tmux 二值模块重建使用两像素比例，将输入解码器的像素数减少 75%，录像路径保持原样。

## 续传和新版本

Ctrl-C 中断后，再执行同一条命令即可继续。中断任务沿用第一次准备的快照，源文件后续变化不会混入这次传输。想放弃旧快照、获取当前内容时：

```bash
kiss-get /home/chivier/results.csv --fresh
```

已完成的任务会检查远端文件是否变化；未变化且本地文件校验一致时直接完成。已有不同内容的同名文件会保留，新文件保存到 `transfer-<ID>/` 子目录。

续传默认沿用原二维码容量。`fast` 和 `turbo` 容量相同，可在续传时互换；跨容量切档位需要 `--fresh`，旧快照和本机断点仍然保留：

```bash
kiss-get /home/chivier/results.csv --speed fast --fresh
```

文件夹使用同一条命令，自动打包成 `.tar` 并在传输层压缩；Mac 收到后保留 tar 归档，按需自行解压。符号链接保留为归档条目，不跟随链接读取其他目录。不要选择包含工具缓存 `~/.cache/tqr-get` 的上级目录作为源。

## 可选设置

```bash
kiss-get /home/chivier/results.csv -o ~/Downloads/my-results
kiss-get /home/chivier/results.csv --fps 10
kiss-get /home/chivier/results.csv --speed fast --fps 20 --capture-fps 80
kiss-get /home/chivier/results.csv --no-ui
```

默认使用 `auto`；`--fps` 可覆盖发送帧率，范围 0.1–60，`--capture-fps` 可覆盖采样率，范围 0.1–120。`--no-ui` 仅显示进度。`--timeout 秒数` 在时限到达后保存断点退出。默认接收上限 1 GiB，可用 `--max-bytes 字节数` 调整，例如传 5 GB 文件时设置 `--max-bytes 6000000000`。

本机状态保存在 `${XDG_STATE_HOME:-~/.local/state}/tqr-get/`，远端快照保存在 `~/.cache/tqr-get/`。这些内容保留用于续传，工具不会自动删除源文件或其他缓存。

## 安装与验证

本机需要 Python 3.9+、`tmux`、完整版本 `tqr` 和可用的 SSH。服务器需要 Python 3.9+ 和已安装的 `tqr` 发送端，无需远端 tmux。使用本机现有 SSH 密钥和别名，遵守 SSH 配置中的 ProxyJump；不传递本机 GitHub 凭据。

```bash
sh scripts/install-get.sh
kiss-get --help
python3 -m unittest discover -s tests -p test_tqr_get.py -v
```

默认安装到 `~/.local/bin`，确保该目录已在 PATH 中。使用 `TQR_GET_BIN_DIR` 可以指定其他目录。
