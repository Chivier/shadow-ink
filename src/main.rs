use anyhow::{ensure, Context, Result};
use clap::{Parser, Subcommand};
use shadow_ink::{
    bundle::{self, Bundle},
    render, sender,
};
use std::{
    io::IsTerminal,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Parser)]
#[command(
    name = "shadow-ink",
    version,
    after_help = "Remote file: shadow-ink SERVER FILEPATH [-o DIRECTORY]\nOptions: shadow-ink get --help",
    about = "Resumable terminal text and QR transfer. Quick start: shadow-ink SERVER FILEPATH"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Snapshot and optionally compress a file into an immutable playback bundle.
    Prepare {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long,default_value="auto",value_parser=["none","auto","zstd"])]
        compress: String,
        #[arg(long, default_value_t = 224)]
        symbol_bytes: u16,
        #[arg(long, default_value = "rs:64:96")]
        fec: String,
    },
    /// Play text or QR frames in the current terminal. q quits; space pauses.
    Send {
        bundle: PathBuf,
        #[arg(long, default_value = "qr", value_parser = ["qr", "text", "text-stream"])]
        backend: String,
        #[arg(long, default_value_t = 12)]
        qr_version: u8,
        #[arg(long,default_value="M",value_parser=["L","M","Q","H"])]
        ecc: String,
        #[arg(long, default_value_t = 3.0)]
        fps: f64,
        #[arg(long = "loop", conflicts_with = "rounds")]
        loop_forever: bool,
        #[arg(long)]
        rounds: Option<u64>,
        #[arg(long)]
        only_generations: Option<String>,
    },
    /// Validate a prepared bundle and show its manifest and transfer identity.
    Inspect { bundle: PathBuf },
    /// Check terminal geometry, pixel aspect and QR capacity; optionally show a pattern.
    Doctor {
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        tmux_socket: Option<PathBuf>,
        #[arg(long)]
        bundle: Option<PathBuf>,
        #[arg(long, default_value_t = 12)]
        qr_version: u8,
        #[arg(long,default_value="M",value_parser=["L","M","Q","H"])]
        ecc: String,
        #[arg(long)]
        cell_pixels: Option<String>,
        #[arg(long)]
        probe_pixels: bool,
        #[arg(long)]
        show_pattern: bool,
    },
    #[cfg(feature = "receiver")]
    /// Recover from a local tmux pane or a recorded video. Never connects to server A.
    Recv {
        #[arg(long)]
        source: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        tmux_socket: Option<PathBuf>,
        #[arg(long, default_value_t = 20.0)]
        capture_fps: f64,
        #[arg(long)]
        timeout: Option<f64>,
        #[arg(long, default_value_t = 1073741824)]
        max_bytes: u64,
        #[arg(long)]
        transfer_id: Option<String>,
        #[arg(long)]
        roi: Option<shadow_ink::sources::Roi>,
    },
    #[cfg(feature = "receiver")]
    /// Show persisted progress, missing generation ranges and reception statistics.
    Status { state: PathBuf },
    #[cfg(feature = "receiver")]
    /// List local tmux pane IDs and usable dimensions.
    Panes {
        #[arg(long)]
        tmux_socket: Option<PathBuf>,
    },
    #[cfg(feature = "receiver")]
    /// Measure local tmux QR observations while the sender plays in another pane.
    Calibrate {
        #[arg(long)]
        source: String,
        #[arg(long)]
        tmux_socket: Option<PathBuf>,
        #[arg(long, default_value_t = 10.0)]
        seconds: f64,
        #[arg(long, default_value_t = 20.0)]
        capture_fps: f64,
    },
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("shadow-ink: {e:#}");
            std::process::exit(1);
        }
    }
}
fn run() -> Result<i32> {
    // Keep the installed executable self-contained: the orchestration helper is
    // embedded, and calls this exact binary for the local receiving process.
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if let Some(first) = args.first().and_then(|a| a.to_str()) {
        let commands = [
            "prepare",
            "send",
            "inspect",
            "doctor",
            "recv",
            "status",
            "panes",
            "calibrate",
            "help",
        ];
        if first == "get" || (!first.starts_with('-') && !commands.contains(&first)) {
            #[cfg(not(feature = "receiver"))]
            anyhow::bail!("remote reception requires the full receiver build");
            #[cfg(feature = "receiver")]
            {
                let mut command = std::process::Command::new("python3");
                command
                    .arg("-c")
                    .arg(include_str!("../scripts/shadow-ink-get"))
                    .args(if first == "get" {
                        &args[1..]
                    } else {
                        &args[..]
                    })
                    .env("SHADOW_INK_BIN", std::env::current_exe()?);
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    return Err(command.exec()).context("start Python 3 receiver helper");
                }
                #[cfg(not(unix))]
                return Ok(command.status()?.code().unwrap_or(1));
            }
        }
    }
    let cli = Cli::parse();
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    match cli.command {
        Cmd::Prepare {
            input,
            out,
            compress,
            symbol_bytes,
            fec,
        } => {
            ensure!(fec == "rs:64:96", "v1 supports only --fec rs:64:96");
            let m = bundle::prepare(&input, &out, &compress, symbol_bytes)?;
            let b = Bundle::open(&out)?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"bundle":out,"transfer_id":hex::encode(b.id),"manifest":m,"generations":b.manifest.generations()})
                )?
            );
        }
        Cmd::Send {
            bundle,
            backend,
            qr_version,
            ecc,
            fps,
            loop_forever: _,
            rounds,
            only_generations,
        } => {
            if backend == "qr" {
                sender::send(
                    &bundle,
                    qr_version,
                    &ecc,
                    fps,
                    only_generations.as_deref(),
                    rounds,
                    stop,
                )?;
            } else {
                sender::send_text(
                    &bundle,
                    fps,
                    backend == "text-stream",
                    only_generations.as_deref(),
                    rounds,
                    stop,
                )?;
            }
        }
        Cmd::Inspect { bundle } => {
            let b = Bundle::open(&bundle)?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"transfer_id":hex::encode(b.id),"manifest":b.manifest,"generations":b.manifest.generations(),"packet_bytes":b.manifest.symbol_bytes as usize+shadow_ink::protocol::OVERHEAD})
                )?
            );
        }
        Cmd::Doctor {
            source,
            tmux_socket,
            bundle,
            qr_version,
            ecc,
            cell_pixels,
            probe_pixels,
            show_pattern,
        } => {
            let mut dimensions = None;
            let mut warnings = vec![];
            if let Some(src) = source.as_deref() {
                #[cfg(feature = "receiver")]
                {
                    let pane = src
                        .strip_prefix("tmux:")
                        .context("doctor source must be tmux:%ID")?;
                    dimensions = Some(shadow_ink::sources::pane_size(
                        pane,
                        tmux_socket.as_deref(),
                    )?);
                }
                #[cfg(not(feature = "receiver"))]
                {
                    let _ = (src, tmux_socket);
                    anyhow::bail!("tmux inspection requires the receiver build");
                }
            } else if std::io::stdout().is_terminal() {
                dimensions = Some(crossterm::terminal::size()?);
            }
            let mut pixels = None;
            let mut pixels_source = "unknown";
            if let Some(text) = cell_pixels {
                let (w, h) = text
                    .split_once('x')
                    .context("cell pixels must be WIDTHxHEIGHT, e.g. 8x16")?;
                let (w, h): (u16, u16) = (w.parse()?, h.parse()?);
                ensure!(
                    w > 0 && h > 0 && w <= 512 && h <= 512,
                    "invalid cell pixels"
                );
                pixels = Some((w, h));
                pixels_source = "explicit";
            } else if source.is_none() {
                if let Ok(size) = crossterm::terminal::window_size() {
                    if size.width > 0 && size.height > 0 && size.columns > 0 && size.rows > 0 {
                        pixels = Some((size.width / size.columns, size.height / size.rows));
                        pixels_source = "window-pixels-estimate";
                    }
                }
                if probe_pixels && std::io::stdout().is_terminal() {
                    if let Some(measured) = probe_cell()? {
                        pixels = Some(measured);
                        pixels_source = "terminal-query";
                    }
                }
            }
            if dimensions.is_none() {
                warnings.push("No attached TTY; actual terminal dimensions are unknown.");
            }
            if pixels.is_none() {
                warnings.push("Cell pixel dimensions are unknown; do not assume the font is 1:2.");
            }
            if pixels.is_some_and(|(w, h)| (2.0 * f64::from(w) / f64::from(h) - 1.0).abs() > 0.1) {
                warnings.push("Half-block modules differ from square by over 10%; check font/line spacing and test the recording.");
            }
            let size = if let Some(p) = bundle {
                Bundle::open(&p)?.manifest.symbol_bytes
            } else {
                224
            };
            let packet_fit = render::check_capacity(size, qr_version, &ecc).is_ok();
            let (w, h) = render::geometry(qr_version)?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"pane_columns_rows":dimensions,"cell_pixels_width_height":pixels,"cell_pixels_source":pixels_source,"module_width_height_ratio":pixels.map(|(w,h)|2.0*f64::from(w)/f64::from(h)),"pixel_geometry_verified":false,"qr_columns_rows":[w,h],"required_columns_rows":[w.max(55)+1,h+3],"pane_fits":dimensions.map(|(c,r)|render::fits(qr_version,c,r)),"packet_bytes":size as usize+60,"packet_fits":packet_fit,"warnings":warnings})
                )?
            );
            if show_pattern {
                sender::preview(qr_version, &ecc, stop)?;
            }
            if !packet_fit || dimensions.is_some_and(|(c, r)| !render::fits(qr_version, c, r)) {
                return Ok(2);
            }
        }
        #[cfg(feature = "receiver")]
        Cmd::Recv {
            source,
            output,
            state,
            tmux_socket,
            capture_fps,
            timeout,
            max_bytes,
            transfer_id,
            roi,
        } => {
            let transfer = transfer_id
                .as_deref()
                .map(shadow_ink::receiver::transfer_id)
                .transpose()?;
            let report = shadow_ink::receiver::receive(
                shadow_ink::receiver::Options {
                    source: &source,
                    state: &state,
                    output: &output,
                    socket: tmux_socket.as_deref(),
                    capture_fps,
                    timeout,
                    max_bytes,
                    transfer,
                    roi,
                },
                stop,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.complete {
                return Ok(2);
            }
        }
        #[cfg(feature = "receiver")]
        Cmd::Status { state } => println!(
            "{}",
            serde_json::to_string_pretty(&shadow_ink::receiver::status(&state)?)?
        ),
        #[cfg(feature = "receiver")]
        Cmd::Panes { tmux_socket } => {
            print!("{}", shadow_ink::sources::panes(tmux_socket.as_deref())?)
        }
        #[cfg(feature = "receiver")]
        Cmd::Calibrate {
            source,
            tmux_socket,
            seconds,
            capture_fps,
        } => println!(
            "{}",
            serde_json::to_string_pretty(&shadow_ink::receiver::calibrate(
                &source,
                tmux_socket.as_deref(),
                seconds,
                capture_fps,
                stop
            )?)?
        ),
    }
    Ok(0)
}

#[cfg(unix)]
fn probe_cell() -> Result<Option<(u16, u16)>> {
    use std::{
        fs::OpenOptions,
        io::{Read, Write},
        os::fd::AsRawFd,
        time::{Duration, Instant},
    };
    let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    crossterm::terminal::enable_raw_mode()?;
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let _reset = Reset;
    tty.write_all(b"\x1b[16t")?;
    tty.flush()?;
    let deadline = Instant::now() + Duration::from_millis(300);
    let mut reply = vec![];
    while Instant::now() < deadline && reply.len() < 128 {
        let mut fd = libc::pollfd {
            fd: tty.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: fd points to one live pollfd; poll has no ownership of the TTY.
        let ready = unsafe { libc::poll(&mut fd, 1, 20) };
        if ready > 0 {
            let mut b = [0; 1];
            tty.read_exact(&mut b)?;
            reply.push(b[0]);
            if b[0] == b't' {
                break;
            }
        }
    }
    let reply = String::from_utf8_lossy(&reply);
    if let Some(s) = reply
        .strip_prefix("\x1b[6;")
        .and_then(|s| s.strip_suffix('t'))
    {
        if let Some((h, w)) = s.split_once(';') {
            if let (Ok(h), Ok(w)) = (h.parse::<u16>(), w.parse::<u16>()) {
                if h > 0 && w > 0 {
                    return Ok(Some((w, h)));
                }
            }
        }
    }
    Ok(None)
}
#[cfg(not(unix))]
fn probe_cell() -> Result<Option<(u16, u16)>> {
    Ok(None)
}
