use crate::{
    protocol::Frame,
    sources::{self, Roi, Video},
    store::{Ingest, Status, Store},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub captures: u64,
    pub qr_codes: u64,
    pub text_lines: u64,
    pub valid_frames: u64,
    pub new_symbols: u64,
    pub new_data_symbols: u64,
    pub new_metadata_symbols: u64,
    pub observed_sender_fps: Option<f64>,
    pub duplicate_symbols: u64,
    pub repeated_captures: u64,
    pub unstable_captures: u64,
    pub crc_failures: u64,
    pub rejected_frames: u64,
    pub conflicting_symbols: u64,
    pub foreign_frames: u64,
    pub observed_forward_sequence_gaps: u64,
    pub last_capture_index: Option<u64>,
    pub last_video_pts: Option<f64>,
    pub elapsed_seconds: f64,
    pub recovered_bytes_per_second: f64,
}

#[derive(Serialize)]
pub struct Report {
    pub complete: bool,
    pub interrupted: bool,
    pub output: Option<PathBuf>,
    pub status: Status,
    pub statistics: Stats,
}

pub struct Options<'a> {
    pub source: &'a str,
    pub state: &'a Path,
    pub output: &'a Path,
    pub socket: Option<&'a Path>,
    pub capture_fps: f64,
    pub timeout: Option<f64>,
    pub max_bytes: u64,
    pub transfer: Option<[u8; 16]>,
    pub roi: Option<Roi>,
}

fn persist(store: &Store, stats: &Stats) -> Result<()> {
    store.conn.execute(
        "INSERT OR REPLACE INTO settings VALUES('statistics',?1)",
        [serde_json::to_vec(stats)?],
    )?;
    Ok(())
}

pub fn receive(options: Options<'_>, stop: Arc<AtomicBool>) -> Result<Report> {
    ensure!(
        options.capture_fps.is_finite() && (0.1..=120.0).contains(&options.capture_fps),
        "capture fps must be 0.1..=120"
    );
    if let Some(t) = options.timeout {
        ensure!(t.is_finite() && t > 0.0, "timeout must be positive");
    }
    let (kind, path) = options
        .source
        .split_once(':')
        .context("source must be tmux:%ID or video:PATH")?;
    ensure!(
        matches!(kind, "tmux" | "video" | "text" | "pipe"),
        "sources: tmux:%ID, text:%ID, pipe:%ID or video:PATH"
    );
    let mut store = Store::open(options.state, options.max_bytes, options.transfer)?;
    let start = Instant::now();
    let baseline = store.status()?.verified_transport_bytes;
    let mut stats = Stats::default();
    let mut output = None;
    if store.ready()? {
        output = Some(store.finalize(options.output)?);
        return Ok(Report {
            complete: true,
            interrupted: false,
            output,
            status: store.status()?,
            statistics: stats,
        });
    }
    let mut video = if kind == "video" {
        Some(Video::open(Path::new(path))?)
    } else {
        sources::pane_size(path, options.socket)?;
        None
    };
    #[cfg(unix)]
    let mut pipe = if kind == "pipe" {
        let pipe = sources::TextPipe::open(path, options.socket)?;
        std::fs::write(options.state.join("pipe-ready"), b"ready")?;
        Some(pipe)
    } else {
        None
    };
    #[cfg(not(unix))]
    ensure!(kind != "pipe", "pipe reception requires Unix");
    // Publish only after Store::open has revalidated cached shards and .part
    // groups. The controller must not skip groups based on stale DB counters.
    if kind != "video" {
        let ready = tempfile::NamedTempFile::new_in(options.state)?;
        serde_json::to_writer(ready.as_file(), &store.status()?)?;
        ready.persist(options.state.join("receiver-ready.json"))?;
    }
    let mut runs: VecDeque<(u64, u64)> = VecDeque::new();
    let mut last_playback = None;
    let mut playback_start: Option<(u64, u64, Instant)> = None;
    let mut last_status = Instant::now();
    let interval = Duration::from_secs_f64(1.0 / options.capture_fps);
    loop {
        if stop.load(Ordering::Relaxed)
            || options
                .timeout
                .is_some_and(|t| start.elapsed().as_secs_f64() >= t)
        {
            break;
        }
        let tick = Instant::now();
        let mut marker = None;
        let packets = if kind == "text" {
            let packets = sources::capture_text(path, options.socket)?;
            stats.text_lines += packets.len() as u64;
            packets
        } else if kind == "pipe" {
            #[cfg(unix)]
            let packets = pipe.as_mut().unwrap().packets()?;
            #[cfg(not(unix))]
            let packets: Vec<Result<Vec<u8>>> = Vec::new();
            stats.text_lines += packets.len() as u64;
            packets
        } else {
            let capture = if let Some(video) = &mut video {
                match video.next_frame()? {
                    Some(c) => Some(c),
                    None => break,
                }
            } else {
                sources::capture_tmux(path, options.socket, stats.captures)?
            };
            if let Some(capture) = capture {
                stats.last_capture_index = Some(capture.index);
                stats.last_video_pts = capture.pts;
                marker = capture.marker;
                let image = if let Some(roi) = options.roi {
                    roi.crop(&capture.image)?
                } else {
                    capture.image
                };
                let bytes = sources::decode(&image)?;
                stats.qr_codes += bytes.len() as u64;
                bytes.into_iter().map(Ok).collect()
            } else {
                stats.unstable_captures += 1;
                Vec::new()
            }
        };
        stats.captures += 1;
        for bytes in packets {
            let bytes = match bytes {
                Ok(b) => b,
                Err(_) => {
                    stats.rejected_frames += 1;
                    continue;
                }
            };
            let frame = match Frame::decode(&bytes) {
                Ok(f) => f,
                Err(e) => {
                    if e.to_string().contains("CRC32C") {
                        stats.crc_failures += 1;
                    } else {
                        stats.rejected_frames += 1;
                    }
                    continue;
                }
            };
            if marker.as_ref().is_some_and(|m| *m != frame.marker(&bytes)) {
                stats.unstable_captures += 1;
                continue;
            }
            stats.valid_frames += 1;
            if options.transfer.is_none_or(|id| id == frame.transfer) {
                match playback_start {
                    Some((run, seq, at)) if run == frame.run && frame.seq > seq => {
                        stats.observed_sender_fps =
                            Some((frame.seq - seq) as f64 / at.elapsed().as_secs_f64());
                    }
                    Some((run, _, _)) if run == frame.run => {}
                    _ => playback_start = Some((frame.run, frame.seq, Instant::now())),
                }
            }
            let identity = (frame.transfer, frame.run, frame.seq);
            if last_playback == Some(identity) {
                stats.repeated_captures += 1;
            }
            last_playback = Some(identity);
            if let Some((_, max)) = runs.iter_mut().find(|(id, _)| *id == frame.run) {
                if frame.seq > *max {
                    stats.observed_forward_sequence_gaps = stats
                        .observed_forward_sequence_gaps
                        .saturating_add(frame.seq.saturating_sub(*max).saturating_sub(1));
                    *max = frame.seq;
                }
            } else {
                if runs.len() >= 64 {
                    runs.pop_front();
                }
                runs.push_back((frame.run, frame.seq));
            }
            match store.ingest(&frame) {
                Ok(Ingest::Duplicate) => stats.duplicate_symbols += 1,
                Ok(Ingest::Ignored) => stats.foreign_frames += 1,
                Ok(Ingest::Conflict) => stats.conflicting_symbols += 1,
                Ok(_) => {
                    stats.new_symbols += 1;
                    if frame.object == 1 {
                        stats.new_data_symbols += 1;
                    } else {
                        stats.new_metadata_symbols += 1;
                    }
                }
                Err(e) => {
                    // Malformed protocol data is rejected. Storage/I/O failures must stop
                    // reception, never be disguised as a harmless bad QR frame.
                    if e.downcast_ref::<crate::store::FrameRejected>().is_none() {
                        return Err(e);
                    }
                    stats.rejected_frames += 1;
                    eprintln!("frame rejected: {e:#}");
                }
            }
            if store.ready()? {
                output = Some(store.finalize(options.output)?);
                break;
            }
        }
        if last_status.elapsed() >= Duration::from_secs(1) || output.is_some() {
            let s = store.status()?;
            stats.elapsed_seconds = start.elapsed().as_secs_f64();
            stats.recovered_bytes_per_second = s.verified_transport_bytes.saturating_sub(baseline)
                as f64
                / stats.elapsed_seconds.max(0.001);
            persist(&store, &stats)?;
            eprintln!(
                "verified groups {}/{} | new={} duplicate={} CRC={} rejected={} | {:.0} B/s",
                s.completed_generations,
                s.total_generations,
                stats.new_symbols,
                stats.duplicate_symbols,
                stats.crc_failures,
                stats.rejected_frames,
                stats.recovered_bytes_per_second
            );
            last_status = Instant::now();
        }
        if output.is_some() {
            break;
        }
        if video.is_none() {
            let remaining = interval.saturating_sub(tick.elapsed());
            let deadline = Instant::now() + remaining;
            while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
                std::thread::sleep(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(50)),
                );
            }
        }
    }
    stats.elapsed_seconds = start.elapsed().as_secs_f64();
    let status = store.status()?;
    stats.recovered_bytes_per_second = status.verified_transport_bytes.saturating_sub(baseline)
        as f64
        / stats.elapsed_seconds.max(0.001);
    persist(&store, &stats)?;
    Ok(Report {
        complete: output.is_some(),
        interrupted: stop.load(Ordering::Relaxed),
        output,
        status,
        statistics: stats,
    })
}

pub fn transfer_id(text: &str) -> Result<[u8; 16]> {
    let bytes = hex::decode(text)?;
    ensure!(bytes.len() == 16, "transfer ID must be 32 hex digits");
    Ok(bytes.try_into().unwrap())
}

pub fn status(state: &Path) -> Result<serde_json::Value> {
    let s = crate::store::status(state)?;
    let mut value = serde_json::to_value(s)?;
    let conn = rusqlite::Connection::open_with_flags(
        state.join("state.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    use rusqlite::OptionalExtension;
    let stats: Option<Vec<u8>> = conn
        .query_row(
            "SELECT value FROM settings WHERE key='statistics'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(raw) = stats {
        value["statistics"] = serde_json::from_slice(&raw)?;
    }
    Ok(value)
}

pub fn calibrate(
    source: &str,
    socket: Option<&Path>,
    seconds: f64,
    fps: f64,
    stop: Arc<AtomicBool>,
) -> Result<serde_json::Value> {
    ensure!(
        seconds.is_finite() && seconds > 0.0 && seconds <= 3600.0,
        "seconds must be 0..=3600"
    );
    ensure!(
        fps.is_finite() && (0.1..=120.0).contains(&fps),
        "capture fps must be 0.1..=120"
    );
    let pane=source.strip_prefix("tmux:").context("calibrate currently measures a local tmux source; video reception reports its own statistics")?;
    let start = Instant::now();
    let mut captures = 0;
    let mut valid = 0;
    let mut unique = std::collections::HashSet::new();
    while start.elapsed().as_secs_f64() < seconds && !stop.load(Ordering::Relaxed) {
        let tick = Instant::now();
        if let Some(c) = sources::capture_tmux(pane, socket, captures)? {
            for bytes in sources::decode(&c.image)? {
                if let Ok(f) = Frame::decode(&bytes) {
                    if c.marker == Some(f.marker(&bytes)) {
                        valid += 1;
                        unique.insert((f.run, f.seq));
                    }
                }
            }
        }
        captures += 1;
        let wait = Duration::from_secs_f64(1.0 / fps).saturating_sub(tick.elapsed());
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
            std::thread::sleep(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(50)),
            );
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    Ok(
        serde_json::json!({"captures":captures,"valid_qr_observations":valid,"unique_playback_frames":unique.len(),"observed_unique_frames_per_second":unique.len() as f64/elapsed,"elapsed_seconds":elapsed,"note":"Measured on the local tmux grid; this does not measure font rendering, video capture or remote acknowledgement."}),
    )
}
