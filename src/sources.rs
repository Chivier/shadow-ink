use anyhow::{bail, ensure, Context, Result};
use image::{GrayImage, Luma};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
};

pub struct Capture {
    pub image: GrayImage,
    pub marker: Option<String>,
    pub index: u64,
    pub pts: Option<f64>,
}

fn tmux(socket: Option<&Path>) -> Command {
    let mut c = Command::new("tmux");
    if let Some(p) = socket {
        c.arg("-S").arg(p);
    }
    c.stdin(Stdio::null());
    c
}

pub fn panes(socket: Option<&Path>) -> Result<String> {
    let out=tmux(socket).args(["list-panes","-a","-F","#{pane_id}\t#{session_name}:#{window_index}.#{pane_index}\t#{pane_width}x#{pane_height}\t#{pane_current_command}"]).output().context("run tmux")?;
    ensure!(
        out.status.success(),
        "tmux: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?)
}

pub fn pane_size(pane: &str, socket: Option<&Path>) -> Result<(u16, u16)> {
    ensure!(
        pane.starts_with('%') && pane.len() > 1 && pane[1..].bytes().all(|b| b.is_ascii_digit()),
        "use an explicit pane ID, such as tmux:%3 (see shadow-ink panes)"
    );
    let out = tmux(socket)
        .args([
            "display-message",
            "-p",
            "-t",
            pane,
            "#{pane_width} #{pane_height}",
        ])
        .output()?;
    ensure!(
        out.status.success(),
        "tmux pane not accessible: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout)?;
    let sizes: Vec<_> = text.split_whitespace().collect();
    ensure!(sizes.len() == 2, "invalid tmux size reply");
    let cols: u16 = sizes[0].parse()?;
    let rows: u16 = sizes[1].parse()?;
    ensure!(
        (1..=1024).contains(&cols) && (1..=512).contains(&rows),
        "tmux pane exceeds capture limits (1024x512)"
    );
    Ok((cols, rows))
}

pub fn capture_tmux(pane: &str, socket: Option<&Path>, index: u64) -> Result<Option<Capture>> {
    ensure!(
        pane.starts_with('%') && pane.len() > 1 && pane[1..].bytes().all(|b| b.is_ascii_digit()),
        "use an explicit pane ID, such as tmux:%3 (see shadow-ink panes)"
    );
    // Queue geometry/capture/geometry in one tmux client. Preserve the resize
    // guard without spawning three clients for every sample.
    let out = tmux(socket)
        .args([
            "display-message",
            "-p",
            "-t",
            pane,
            "#{pane_width} #{pane_height}",
            ";",
            "capture-pane",
            "-p",
            "-e",
            "-N",
            "-t",
            pane,
            ";",
            "display-message",
            "-p",
            "-t",
            pane,
            "#{pane_width} #{pane_height}",
        ])
        .output()?;
    ensure!(
        out.status.success(),
        "tmux capture failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    ensure!(
        out.stdout.len() <= 4 * 1024 * 1024,
        "tmux capture too large"
    );
    capture_tmux_reply(&out.stdout, index)
}

fn capture_tmux_reply(reply: &[u8], index: u64) -> Result<Option<Capture>> {
    let reply = std::str::from_utf8(reply)?.trim_end_matches('\n');
    let (before, rest) = reply.split_once('\n').context("missing capture geometry")?;
    let (dump, after) = rest
        .rsplit_once('\n')
        .context("missing final capture geometry")?;
    if before != after {
        return Ok(None);
    }
    let sizes: Vec<_> = before.split_whitespace().collect();
    ensure!(sizes.len() == 2, "invalid capture geometry");
    let (cols, rows) = (sizes[0].parse::<u16>()?, sizes[1].parse::<u16>()?);
    tmux_image(dump.as_bytes(), cols, rows).map(|v| {
        v.map(|(image, marker)| Capture {
            image,
            marker: Some(marker),
            index,
            pts: None,
        })
    })
}

pub fn tmux_image(bytes: &[u8], cols: u16, rows: u16) -> Result<Option<(GrayImage, String)>> {
    // capture-pane emits LF-separated grid rows, not a live VT byte stream.
    // Use CRLF explicitly, omitting the final LF to prevent an artificial scroll.
    let text = std::str::from_utf8(bytes)?;
    let lines: Vec<_> = text
        .strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect();
    if lines.len() > rows as usize {
        return Ok(None);
    }
    ensure!(
        cols > 0 && cols <= 1024 && rows > 0 && rows <= 512,
        "invalid grid dimensions"
    );
    let mut parser = vt100::Parser::new(
        std::num::NonZeroU16::new(rows + 1).unwrap(),
        std::num::NonZeroU16::new(cols + 1).unwrap(),
        0,
    );
    parser.process(lines.join("\r\n").as_bytes());
    let screen = parser.screen();
    let row_text = |r: u16| -> String {
        (0..cols)
            .filter_map(|c| screen.cell(r, c))
            .map(|c| c.contents())
            .collect::<String>()
            .trim_end()
            .to_owned()
    };
    let top = row_text(0);
    if !top.starts_with("TQ1 ") {
        return Ok(None);
    }
    let footer = (1..rows).find(|&r| row_text(r) == top);
    let Some(footer) = footer else {
        return Ok(None);
    };
    if footer < 3 {
        return Ok(None);
    }
    let q = 2 * (footer - 1) - 1;
    if !(29..=185).contains(&q) || (q - 25) % 4 != 0 || q >= cols {
        return Ok(None);
    }
    // tmux supplies exact binary modules, unlike a recording. Two pixels per
    // module retain the quiet zone while reducing decoder pixels by 75%.
    const SCALE: u32 = 2;
    let mut image = GrayImage::new(u32::from(q) * SCALE, u32::from(q) * SCALE);
    for y in 0..q {
        for x in 0..q {
            let cell = screen.cell(1 + y / 2, x).context("missing terminal cell")?;
            if cell.is_wide() || cell.is_wide_continuation() {
                return Ok(None);
            }
            let (mut fg, mut bg) = (cell.fgcolor(), cell.bgcolor());
            if cell.inverse() {
                std::mem::swap(&mut fg, &mut bg);
            }
            let use_fg = match cell.contents() {
                "▀" => y % 2 == 0,
                "▄" => y % 2 == 1,
                "█" => true,
                " " | "" => false,
                _ => return Ok(None),
            };
            let Some(value) = colour(if use_fg { fg } else { bg }) else {
                return Ok(None);
            };
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    image.put_pixel(
                        u32::from(x) * SCALE + dx,
                        u32::from(y) * SCALE + dy,
                        Luma([value]),
                    );
                }
            }
        }
    }
    Ok(Some((image, top)))
}

fn colour(c: vt100::Color) -> Option<u8> {
    match c {
        vt100::Color::Rgb(r, g, b) => Some(
            if u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 >= 128000 {
                255
            } else {
                0
            },
        ),
        vt100::Color::Idx(0 | 8) => Some(0),
        vt100::Color::Idx(7 | 15) => Some(255),
        vt100::Color::Idx(n) if n >= 232 => Some(if n >= 244 { 255 } else { 0 }),
        vt100::Color::Idx(n) if n >= 16 => {
            let n = u32::from(n - 16);
            let r = n / 36;
            let g = (n / 6) % 6;
            let b = n % 6;
            Some(if r * 299 + g * 587 + b * 114 >= 2500 {
                255
            } else {
                0
            })
        }
        _ => None,
    }
}

pub struct Video {
    child: Child,
    stdout: BufReader<ChildStdout>,
    thread: Option<JoinHandle<()>>,
    pts: mpsc::Receiver<f64>,
    errors: Arc<Mutex<String>>,
    index: u64,
    finished: bool,
}

impl Video {
    pub fn open(path: &Path) -> Result<Self> {
        ensure!(
            fs::metadata(path)?.is_file(),
            "video source must be a regular local file"
        );
        let path = fs::canonicalize(path)?;
        let mut child = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "info",
                "-protocol_whitelist",
                "file,pipe",
                "-i",
            ])
            .arg(path)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-vf",
                "showinfo=checksum=0",
                "-fps_mode",
                "passthrough",
                "-f",
                "image2pipe",
                "-vcodec",
                "pgm",
                "-pix_fmt",
                "gray",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("ffmpeg is required for video reception")?;
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let stderr = child.stderr.take().unwrap();
        let errors = Arc::new(Mutex::new(String::new()));
        let copy = errors.clone();
        let (tx, pts) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.contains("Parsed_showinfo") {
                    if let Some((_, tail)) = line.split_once("pts_time:") {
                        if let Some(value) = tail
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse::<f64>().ok())
                        {
                            let _ = tx.send(value);
                        }
                    }
                }
                let mut buf = copy.lock().unwrap();
                buf.push_str(&line);
                buf.push('\n');
                if buf.len() > 8192 {
                    let mut cut = buf.len() - 8192;
                    while !buf.is_char_boundary(cut) {
                        cut += 1;
                    }
                    buf.drain(..cut);
                }
            }
        });
        Ok(Self {
            child,
            stdout,
            thread: Some(thread),
            pts,
            errors,
            index: 0,
            finished: false,
        })
    }
    pub fn next_frame(&mut self) -> Result<Option<Capture>> {
        if self.finished {
            return Ok(None);
        }
        let Some(magic) = token(&mut self.stdout)? else {
            self.finished = true;
            let status = self.child.wait()?;
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            ensure!(
                status.success(),
                "ffmpeg failed: {}",
                self.errors.lock().unwrap()
            );
            return Ok(None);
        };
        ensure!(magic == "P5", "unexpected ffmpeg image stream");
        let width: u32 = token(&mut self.stdout)?
            .context("missing video width")?
            .parse()?;
        let height: u32 = token(&mut self.stdout)?
            .context("missing video height")?
            .parse()?;
        ensure!(
            token(&mut self.stdout)?.as_deref() == Some("255"),
            "unsupported video pixel depth"
        );
        ensure!(
            (16..=16384).contains(&width)
                && (16..=16384).contains(&height)
                && u64::from(width) * u64::from(height) <= 32 * 1024 * 1024,
            "video frame exceeds 32 megapixel limit"
        );
        let mut pixels = vec![0; (width as usize) * (height as usize)];
        self.stdout
            .read_exact(&mut pixels)
            .context("truncated video frame")?;
        let pts = self
            .pts
            .recv_timeout(std::time::Duration::from_secs(1))
            .ok();
        let index = self.index;
        self.index += 1;
        Ok(Some(Capture {
            image: GrayImage::from_raw(width, height, pixels).unwrap(),
            marker: None,
            index,
            pts,
        }))
    }
}
impl Drop for Video {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn token(r: &mut impl BufRead) -> Result<Option<String>> {
    let mut token = vec![];
    loop {
        let mut b = [0];
        if r.read(&mut b)? == 0 {
            if token.is_empty() {
                return Ok(None);
            }
            bail!("truncated PGM header");
        }
        if b[0] == b'#' && token.is_empty() {
            let mut line = String::new();
            r.read_line(&mut line)?;
            continue;
        }
        if b[0].is_ascii_whitespace() {
            if !token.is_empty() {
                return Ok(Some(String::from_utf8(token)?));
            }
        } else {
            token.push(b[0]);
            ensure!(token.len() < 32, "invalid PGM header");
        }
    }
}

pub fn decode(image: &GrayImage) -> Result<Vec<Vec<u8>>> {
    let view = zxingcpp::ImageView::from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
        zxingcpp::ImageFormat::Lum,
    )?;
    let barcodes = zxingcpp::read()
        .formats([zxingcpp::BarcodeFormat::QRCode])
        .try_harder(true)
        .try_rotate(true)
        .try_invert(true)
        .max_number_of_symbols(16)
        .from(view)?;
    Ok(barcodes.into_iter().map(|b| b.bytes()).collect())
}

#[derive(Clone, Copy)]
pub struct Roi {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl std::str::FromStr for Roi {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let p: Vec<u32> = s
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()?;
        ensure!(
            p.len() == 4 && p[2] > 0 && p[3] > 0,
            "ROI must be X,Y,WIDTH,HEIGHT"
        );
        Ok(Self {
            x: p[0],
            y: p[1],
            width: p[2],
            height: p[3],
        })
    }
}
impl Roi {
    pub fn crop(&self, img: &GrayImage) -> Result<GrayImage> {
        ensure!(
            self.x
                .checked_add(self.width)
                .is_some_and(|x| x <= img.width())
                && self
                    .y
                    .checked_add(self.height)
                    .is_some_and(|y| y <= img.height()),
            "ROI is outside video frame"
        );
        Ok(image::imageops::crop_imm(img, self.x, self.y, self.width, self.height).to_image())
    }
}

/// Plain text sampling accepts intact rows even while other rows are redrawn.
pub fn capture_text(pane: &str, socket: Option<&Path>) -> Result<Vec<Result<Vec<u8>>>> {
    let out = tmux(socket)
        .args(["capture-pane", "-p", "-t", pane])
        .output()?;
    ensure!(
        out.status.success(),
        "tmux text capture failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    ensure!(
        out.stdout.len() <= 4 * 1024 * 1024,
        "tmux capture too large"
    );
    Ok(out
        .stdout
        .split(|&b| b == b'\n')
        .filter_map(crate::text::decode_line)
        .collect())
}

#[cfg(unix)]
pub struct TextPipe {
    fifo: fs::File,
    parser: crate::text::Lines,
    pane: String,
    socket: Option<std::path::PathBuf>,
    _directory: tempfile::TempDir,
}
#[cfg(unix)]
impl TextPipe {
    pub fn open(pane: &str, socket: Option<&Path>) -> Result<Self> {
        use std::os::unix::{ffi::OsStrExt, fs::OpenOptionsExt};
        pane_size(pane, socket)?;
        let out = tmux(socket)
            .args(["display-message", "-p", "-t", pane, "#{pane_pipe}"])
            .output()?;
        ensure!(
            out.status.success() && out.stdout.starts_with(b"0"),
            "pane already has a pipe; refusing to replace it"
        );
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("text.fifo");
        let name = std::ffi::CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: name is a live NUL-terminated path in our private directory.
        ensure!(
            unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0,
            "create text FIFO: {}",
            std::io::Error::last_os_error()
        );
        // RDWR keeps the FIFO alive before tmux starts its writer. Nonblocking
        // reads keep timeouts and Ctrl-C responsive even if SSH stops producing.
        let fifo = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)?;
        let command = format!(
            "exec cat > '{}'",
            path.to_string_lossy().replace('\'', "'\\''")
        );
        let out = tmux(socket)
            .args(["pipe-pane", "-O", "-o", "-t", pane, &command])
            .output()?;
        ensure!(
            out.status.success(),
            "tmux pipe-pane failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(Self {
            fifo,
            parser: crate::text::Lines::default(),
            pane: pane.to_owned(),
            socket: socket.map(Path::to_path_buf),
            _directory: directory,
        })
    }
    pub fn packets(&mut self) -> Result<Vec<Result<Vec<u8>>>> {
        let mut packets = Vec::new();
        let mut bytes = [0; 65536];
        for _ in 0..16 {
            match self.fifo.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => packets.extend(self.parser.push(&bytes[..n])),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(packets)
    }
}
#[cfg(unix)]
impl Drop for TextPipe {
    fn drop(&mut self) {
        let _ = tmux(self.socket.as_deref())
            .args(["pipe-pane", "-t", &self.pane])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_qr_and_tmux_grid() {
        let f = crate::protocol::tests::frame();
        let bytes = f.encode().unwrap();
        let image = crate::render::image(&bytes, 12, "M", 4).unwrap();
        assert_eq!(decode(&image).unwrap(), vec![bytes.clone()]);
        // A terminal dump (LF rows and SGR) reconstructed from the actual VT renderer.
        let screen = crate::render::terminal(&f, 12, "M", 1).unwrap();
        let dump = screen.replace("\r\n", "\n");
        let (image, marker) = tmux_image(dump.as_bytes(), 120, 40).unwrap().unwrap();
        assert_eq!(decode(&image).unwrap(), vec![bytes.clone()]);
        assert_eq!(marker, f.marker(&bytes));
        let torn = dump.replacen("TQ1 ", "TQ2 ", 1);
        assert!(tmux_image(torn.as_bytes(), 120, 40).unwrap().is_none());
        assert!("0,0,0,2".parse::<Roi>().is_err());
    }

    #[test]
    fn batched_capture_and_large_symbols() {
        for (version, size, ecc) in [(12, 224, "M"), (22, 712, "M"), (40, 2893, "L")] {
            let mut frame = crate::protocol::tests::frame();
            frame.symbol_bytes = size;
            frame.payload = (0..usize::from(size)).map(|i| (i * 131) as u8).collect();
            let bytes = frame.encode().unwrap();
            let (width, height) = crate::render::geometry(version).unwrap();
            let cols = width + 1;
            let rows = height + 3;
            let dump = crate::render::terminal(&frame, version, ecc, 1)
                .unwrap()
                .replace("\r\n", "\n");
            let reply = format!("{cols} {rows}\n{dump}\n{cols} {rows}\n");
            let capture = capture_tmux_reply(reply.as_bytes(), 7).unwrap().unwrap();
            assert_eq!(capture.index, 7);
            assert_eq!(decode(&capture.image).unwrap(), vec![bytes]);
            let resized = format!("{cols} {rows}\n{dump}\n{} {rows}\n", cols + 1);
            assert!(capture_tmux_reply(resized.as_bytes(), 0).unwrap().is_none());
        }
    }

    #[test]
    fn low_entropy_v40_snapshots() {
        for seq in 0..12 {
            let mut frame = crate::protocol::tests::frame();
            frame.seq = seq;
            frame.symbol_bytes = 2893;
            frame.payload = vec![0; 2893];
            let bytes = frame.encode().unwrap();
            let dump = crate::render::terminal(&frame, 40, "L", 1)
                .unwrap()
                .replace("\r\n", "\n");
            let (image, _) = tmux_image(dump.as_bytes(), 186, 96).unwrap().unwrap();
            assert_eq!(decode(&image).unwrap(), vec![bytes], "sequence {seq}");
        }
    }
}
