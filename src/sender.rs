use crate::{
    bundle::{manifest_fec, Bundle},
    protocol::{parse_generations, Frame},
    render,
};
use anyhow::{ensure, Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::{seq::SliceRandom, Rng, SeedableRng};
use std::{
    io::{self, IsTerminal, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub struct Schedule {
    pub bundle: Bundle,
    ids: Option<Vec<u32>>,
    groups: Vec<(u32, Vec<Vec<u8>>)>,
    order: Vec<(usize, u16)>,
    order_at: usize,
    next_window: usize,
    manifest: Vec<Vec<u8>>,
    mk: u8,
    mn: u8,
    manifest_at: usize,
    startup_left: usize,
    data_since_manifest: usize,
    run: u64,
    seq: u64,
    pub round: u64,
    rounds: Option<u64>,
    rng: rand::rngs::StdRng,
}

impl Schedule {
    pub fn new(
        bundle: Bundle,
        only: Option<&str>,
        rounds: Option<u64>,
        seed: Option<u64>,
    ) -> Result<Self> {
        ensure!(rounds != Some(0), "rounds must be positive");
        let ids = only
            .map(|spec| parse_generations(spec, bundle.manifest.generations()))
            .transpose()?;
        let (mk, mn, manifest) = manifest_fec(&bundle.raw, bundle.manifest.symbol_bytes)?;
        let mut rng = match seed {
            Some(s) => rand::rngs::StdRng::seed_from_u64(s),
            None => rand::rngs::StdRng::from_entropy(),
        };
        let run = rng.gen();
        let startup_left = manifest.len() * 2;
        Ok(Self {
            bundle,
            ids,
            groups: vec![],
            order: vec![],
            order_at: 0,
            next_window: 0,
            manifest,
            mk,
            mn,
            manifest_at: 0,
            startup_left,
            data_since_manifest: 0,
            run,
            seq: 0,
            round: 1,
            rounds,
            rng,
        })
    }
    fn frame(
        &mut self,
        object: u32,
        generation: u32,
        symbol: u16,
        k: u8,
        n: u8,
        payload: Vec<u8>,
    ) -> Result<Frame> {
        let seq = self.seq;
        self.seq = self
            .seq
            .checked_add(1)
            .context("playback sequence exhausted")?;
        Ok(Frame {
            transfer: self.bundle.id,
            object,
            generation,
            symbol,
            k,
            n,
            symbol_bytes: self.bundle.manifest.symbol_bytes,
            run: self.run,
            seq,
            payload,
        })
    }
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        if self.startup_left > 0 || self.data_since_manifest >= 19 {
            if self.startup_left > 0 {
                self.startup_left -= 1;
            } else {
                self.data_since_manifest = 0;
            }
            let symbol = self.manifest_at % self.manifest.len();
            self.manifest_at += 1;
            return Ok(Some(self.frame(
                0,
                0,
                symbol as u16,
                self.mk,
                self.mn,
                self.manifest[symbol].clone(),
            )?));
        }
        if self.order_at == self.order.len() {
            let count = self
                .ids
                .as_ref()
                .map_or(self.bundle.manifest.generations() as usize, Vec::len);
            if self.next_window == count {
                if self.rounds.is_some_and(|n| self.round >= n) {
                    return Ok(None);
                }
                self.round = self
                    .round
                    .checked_add(1)
                    .context("round counter exhausted")?;
                self.next_window = 0;
            }
            self.groups.clear();
            self.order.clear();
            self.order_at = 0;
            let end = (self.next_window + 8).min(count);
            let window: Vec<_> = if let Some(ids) = &self.ids {
                ids[self.next_window..end].to_vec()
            } else {
                (self.next_window as u32..end as u32).collect()
            };
            for g in window {
                self.groups.push((g, self.bundle.shards(g)?));
            }
            self.next_window = end;
            // Preserve systematic-first transmission, but change symbol and group
            // order on every window/round to avoid fixed-rate sampling aliasing.
            for range in [0u16..64, 64u16..96] {
                let mut symbols: Vec<_> = range.collect();
                symbols.shuffle(&mut self.rng);
                for s in symbols {
                    let mut gs: Vec<_> = (0..self.groups.len()).collect();
                    gs.shuffle(&mut self.rng);
                    self.order.extend(gs.into_iter().map(|g| (g, s)));
                }
            }
        }
        let (g, s) = self.order[self.order_at];
        self.order_at += 1;
        self.data_since_manifest += 1;
        let (generation, shards) = &self.groups[g];
        Ok(Some(self.frame(
            1,
            *generation,
            s,
            64,
            96,
            shards[s as usize].clone(),
        )?))
    }
}

pub(crate) struct TerminalGuard;
impl TerminalGuard {
    pub(crate) fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let guard = Self;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(guard)
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            crossterm::style::ResetColor,
            Show,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

pub fn send(
    path: &Path,
    version: u8,
    ecc: &str,
    fps: f64,
    only: Option<&str>,
    rounds: Option<u64>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    ensure!(
        fps.is_finite() && (0.1..=60.0).contains(&fps),
        "fps must be 0.1..=60"
    );
    ensure!(
        io::stdout().is_terminal(),
        "send requires a real terminal (run it in the left tmux pane)"
    );
    let bundle = Bundle::open(path)?;
    render::check_capacity(bundle.manifest.symbol_bytes,version,ecc).context("packet does not fit fixed QR version; enlarge the pane/version or prepare with smaller symbols")?;
    let (cols, rows) = terminal::size()?;
    ensure!(
        render::fits(version, cols, rows),
        "pane {cols}x{rows} is too small for QR V{version}; V12 needs at least 74x40 usable cells"
    );
    let mut schedule = Schedule::new(bundle, only, rounds, None)?;
    let _guard = TerminalGuard::enter()?;
    let hold = Duration::from_secs_f64(1.0 / fps);
    let mut paused = false;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let (cols, rows) = terminal::size()?;
        if !render::fits(version, cols, rows) || paused {
            write!(io::stdout(),"\x1b[H\x1b[0m\x1b[2JPaused: pane={cols}x{rows}, needs QR V{version} + 3 rows; space=resume q=quit")?;
            io::stdout().flush()?;
            if input(Duration::from_millis(100), &mut paused)? {
                break;
            }
            continue;
        }
        let Some(frame) = schedule.next_frame()? else {
            break;
        };
        let screen = render::terminal(&frame, version, ecc, schedule.round)?;
        io::stdout().write_all(screen.as_bytes())?;
        io::stdout().flush()?;
        // Hold time begins after writing; slow SSH output is never followed by a catch-up burst.
        let deadline = Instant::now() + hold;
        while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
            if input(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
                &mut paused,
            )? {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn input(timeout: Duration, paused: &mut bool) -> Result<bool> {
    #[cfg(unix)]
    {
        let mut tty = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: tty is one live pollfd; poll does not take ownership of stdin.
        let ready = unsafe { libc::poll(&mut tty, 1, 0) };
        if ready > 0 && tty.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            return Ok(true);
        }
    }

    if event::poll(timeout)? {
        if let Event::Key(k) = event::read()? {
            if k.kind == KeyEventKind::Press {
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
                    KeyCode::Char(' ') => *paused = !*paused,
                    _ => {}
                }
            }
        }
    }
    Ok(false)
}

pub fn preview(version: u8, ecc: &str, stop: Arc<AtomicBool>) -> Result<()> {
    ensure!(
        io::stdout().is_terminal(),
        "calibration pattern requires a terminal"
    );
    let (cols, rows) = terminal::size()?;
    ensure!(
        render::fits(version, cols, rows),
        "pane too small for calibration pattern"
    );
    render::check_capacity(224, version, ecc)?;
    let frame = Frame {
        transfer: crate::protocol::transfer_id(b"tqr calibration pattern"),
        object: 1,
        generation: 0,
        symbol: 0,
        k: 64,
        n: 96,
        symbol_bytes: 224,
        run: rand::random(),
        seq: 0,
        payload: (0..224).map(|i| (i * 29) as u8).collect(),
    };
    let _guard = TerminalGuard::enter()?;
    io::stdout().write_all(render::terminal(&frame, version, ecc, 0)?.as_bytes())?;
    io::stdout().flush()?;
    let mut paused = false;
    while !stop.load(Ordering::Relaxed) {
        if input(Duration::from_millis(100), &mut paused)? {
            break;
        }
    }
    Ok(())
}

/// Text screen: one independently verifiable packet per row. Stream mode uses
/// longer logical lines; tmux pipe-pane observes them before terminal wrapping.
pub fn send_text(
    path: &Path,
    fps: f64,
    stream: bool,
    only: Option<&str>,
    rounds: Option<u64>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    ensure!(
        fps.is_finite() && (0.1..=60.0).contains(&fps),
        "fps must be 0.1..=60"
    );
    ensure!(io::stdout().is_terminal(), "send requires a real terminal");
    let bundle = Bundle::open(path)?;
    let width = crate::text::PREFIX.len()
        + (usize::from(bundle.manifest.symbol_bytes) + crate::protocol::OVERHEAD).div_ceil(3) * 4;
    if !stream {
        let (cols, rows) = terminal::size()?;
        ensure!(width < usize::from(cols) && rows >= 4, "text packet needs at least {} columns and 4 rows; prepare with smaller --symbol-bytes or use text-stream + pipe", width + 1);
    }
    let mut schedule = Schedule::new(bundle, only, rounds, None)?;
    let _guard = TerminalGuard::enter()?;
    let mut paused = false;
    let hold = Duration::from_secs_f64(1.0 / fps);
    let mut stdout = io::stdout().lock();
    while !stop.load(Ordering::Relaxed) {
        let (cols, rows) = terminal::size()?;
        if paused || (!stream && (width >= usize::from(cols) || rows < 4)) {
            if input(Duration::from_millis(100), &mut paused)? {
                break;
            }
            continue;
        }
        let mut screen = if stream {
            String::new()
        } else {
            "\x1b[H".to_owned()
        };
        let count = if stream { 16 } else { usize::from(rows - 2) };
        for row in 0..count {
            let Some(frame) = schedule.next_frame()? else {
                stdout.write_all(screen.as_bytes())?;
                stdout.flush()?;
                return Ok(());
            };
            if !stream {
                screen.push_str(&format!("\x1b[{};1H", row + 1));
            }
            screen.push_str(&crate::text::line(&frame)?);
            if stream {
                screen.push_str("\r\n");
            } else {
                screen.push_str("\x1b[K");
            }
        }
        stdout.write_all(screen.as_bytes())?;
        stdout.flush()?;
        let deadline = Instant::now() + hold;
        while Instant::now() < deadline && !stop.load(Ordering::Relaxed) {
            if input(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
                &mut paused,
            )? {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_identity_and_round_order() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a");
        std::fs::write(&src, b"a").unwrap();
        let p = d.path().join("a.tqtx");
        crate::bundle::prepare(&src, &p, "none", 224).unwrap();
        let mut s = Schedule::new(Bundle::open(&p).unwrap(), None, Some(2), Some(1)).unwrap();
        let mut frames = vec![];
        while let Some(f) = s.next_frame().unwrap() {
            frames.push(f);
        }
        assert!(frames.windows(2).all(|p| p[1].seq == p[0].seq + 1));
        let data: Vec<_> = frames.iter().filter(|f| f.object == 1).collect();
        assert_eq!(data.len(), 192);
        assert_ne!(
            data[..96].iter().map(|f| f.symbol).collect::<Vec<_>>(),
            data[96..].iter().map(|f| f.symbol).collect::<Vec<_>>()
        );
        let mut restart = Schedule::new(Bundle::open(&p).unwrap(), None, Some(1), Some(2)).unwrap();
        let f = restart.next_frame().unwrap().unwrap();
        assert_ne!(f.run, frames[0].run);
        assert_eq!(f.transfer, frames[0].transfer);
    }
}
