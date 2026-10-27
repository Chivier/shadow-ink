//! Real external-tool acceptance. Run with:
//! cargo test --test acceptance -- --ignored --nocapture --test-threads=1
#![cfg(feature = "receiver")]
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tqr::{
    bundle::{self, Bundle},
    sender::Schedule,
    sources,
};

fn bin() -> PathBuf {
    std::env::var_os("TQR_ACCEPTANCE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_tqr")))
}
fn run(args: &[&str]) -> std::process::Output {
    Command::new(bin()).args(args).output().unwrap()
}
fn text(p: &Path) -> &str {
    p.to_str().unwrap()
}
fn evidence(name: &str, value: serde_json::Value) {
    let label = std::env::var("TQR_ACCEPTANCE_LABEL").unwrap_or_default();
    assert!(label
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    let name = if label.is_empty() {
        name.to_owned()
    } else {
        format!("{name}-{label}")
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("acceptance-output");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}

struct Tmux {
    socket: PathBuf,
}
impl Tmux {
    fn command(&self, args: &[&str]) -> String {
        let o = Command::new("tmux")
            .arg("-S")
            .arg(&self.socket)
            .args(["-f", "/dev/null"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "tmux {args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    }
}
impl Drop for Tmux {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .arg("-S")
            .arg(&self.socket)
            .arg("kill-server")
            .output();
    }
}

fn playback(tm: &Tmux, pane: &str, packed: &Path, cli: &Path) {
    let sender = std::env::var_os("TQR_ACCEPTANCE_SENDER")
        .map(PathBuf::from)
        .unwrap_or_else(|| cli.to_owned());
    let prefix: Vec<String> = serde_json::from_str(
        &std::env::var("TQR_ACCEPTANCE_PLAYBACK_PREFIX").unwrap_or_else(|_| "[]".into()),
    )
    .unwrap();
    let mut args: Vec<String> = ["respawn-pane", "-k", "-t", pane]
        .into_iter()
        .map(str::to_owned)
        .collect();
    args.extend(prefix);
    args.extend(
        [text(&sender), "send", text(packed), "--fps", "20", "--loop"]
            .into_iter()
            .map(str::to_owned),
    );
    tm.command(&args.iter().map(String::as_str).collect::<Vec<_>>());
}
fn wait_file(path: &Path, seconds: u64) {
    let end = Instant::now() + Duration::from_secs(seconds);
    while !path.exists() {
        assert!(
            Instant::now() < end,
            "timeout waiting for {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "requires tmux and ffmpeg; exercises actual PTYs and subprocesses"]
fn real_two_pane_playback_restart_and_resize() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("源文件.bin");
    let input: Vec<u8> = (0..20000)
        .map(|i| ((i * 131 + i / 251) % 256) as u8)
        .collect();
    fs::write(&source, &input).unwrap();
    let packed = d.path().join("prepared.tqtx");
    bundle::prepare(&source, &packed, "none", 224).unwrap();
    let tm = Tmux {
        socket: d.path().join("tmux.sock"),
    };
    let left = tm.command(&[
        "new-session",
        "-d",
        "-P",
        "-F",
        "#{pane_id}",
        "-x",
        "244",
        "-y",
        "45",
        "-s",
        "tqr-acceptance",
    ]);
    tm.command(&["set-option", "-g", "window-size", "manual"]);
    let right = tm.command(&["split-window", "-h", "-P", "-F", "#{pane_id}", "-t", &left]);
    let window = tm.command(&["display-message", "-p", "-t", &left, "#{window_id}"]);
    tm.command(&["set-option", "-w", "-t", &window, "remain-on-exit", "on"]);
    let cli = bin();
    playback(&tm, &left, &packed, &cli);
    let state = d.path().join("state");
    let out = d.path().join("received");
    let harness = d.path().join("receiver.py");
    fs::write(&harness,"import subprocess,sys\nwith open(sys.argv[1], 'w') as out, open(sys.argv[2], 'w') as err:\n p=subprocess.Popen(sys.argv[4:],stdout=out,stderr=err)\n with open(sys.argv[3]+'.pid','w') as record: record.write(str(p.pid))\n code=p.wait()\nwith open(sys.argv[3], 'w') as result: result.write(str(code))\n").unwrap();
    let first = d.path().join("first.json");
    let firstlog = d.path().join("first.log");
    let firstexit = d.path().join("first.exit");
    let src = format!("tmux:{left}");
    let panes = Command::new(&cli)
        .args(["panes", "--tmux-socket", text(&tm.socket)])
        .output()
        .unwrap();
    assert!(panes.status.success());
    let pane_list = String::from_utf8(panes.stdout).unwrap();
    assert!(pane_list.contains(&left) && pane_list.contains(&right));
    tm.command(&[
        "respawn-pane",
        "-k",
        "-t",
        &right,
        "python3",
        text(&harness),
        text(&first),
        text(&firstlog),
        text(&firstexit),
        text(&cli),
        "recv",
        "--source",
        &src,
        "--tmux-socket",
        text(&tm.socket),
        "--state",
        text(&state),
        "--output",
        text(&out),
        "--capture-fps",
        "80",
        "--timeout",
        "1.5",
    ]);
    wait_file(&firstexit, 10);
    assert_eq!(
        fs::read_to_string(&firstexit).unwrap(),
        "2",
        "{}",
        fs::read_to_string(&firstlog).unwrap()
    );
    let first_report: serde_json::Value =
        serde_json::from_slice(&fs::read(&first).unwrap()).unwrap();
    assert!(
        first_report["statistics"]["valid_frames"].as_u64().unwrap() > 0,
        "{first_report}"
    );
    assert!(!first_report["complete"].as_bool().unwrap());
    assert!(first_report["status"]["cached_symbols"].as_u64().unwrap() > 0);
    let captured_run = first_report["statistics"]["valid_frames"].clone();
    // The live sender must stop rendering QR frames when a pane becomes too short.
    tm.command(&["resize-window", "-t", &window, "-y", "30"]);
    thread::sleep(Duration::from_millis(300));
    let paused = tm.command(&["capture-pane", "-p", "-t", &left]);
    assert!(paused.contains("Paused:"), "{paused}");
    tm.command(&["resize-window", "-t", &window, "-y", "45"]);
    // Kill only the receiver child created by this test's harness.
    let crash = d.path().join("crash.json");
    let crashlog = d.path().join("crash.log");
    let crashexit = d.path().join("crash.exit");
    let crashpid = d.path().join("crash.exit.pid");
    tm.command(&[
        "respawn-pane",
        "-k",
        "-t",
        &right,
        "python3",
        text(&harness),
        text(&crash),
        text(&crashlog),
        text(&crashexit),
        text(&cli),
        "recv",
        "--source",
        &src,
        "--tmux-socket",
        text(&tm.socket),
        "--state",
        text(&state),
        "--output",
        text(&out),
        "--capture-fps",
        "80",
        "--timeout",
        "35",
    ]);
    wait_file(&crashpid, 5);
    thread::sleep(Duration::from_millis(250));
    assert!(
        !crashexit.exists(),
        "receiver finished before the crash test"
    );
    let pid = fs::read_to_string(crashpid).unwrap();
    let killed = Command::new("python3")
        .args(["-c", "import os,sys; os.kill(int(sys.argv[1]),9)", &pid])
        .status()
        .unwrap();
    assert!(killed.success());
    wait_file(&crashexit, 5);
    assert_eq!(fs::read_to_string(crashexit).unwrap(), "-9");
    // Restart A, keeping the exact prepared bundle. Its run ID changes; B's state survives.
    playback(&tm, &left, &packed, &cli);
    let second = d.path().join("second.json");
    let secondlog = d.path().join("second.log");
    let secondexit = d.path().join("second.exit");
    tm.command(&[
        "respawn-pane",
        "-k",
        "-t",
        &right,
        "python3",
        text(&harness),
        text(&second),
        text(&secondlog),
        text(&secondexit),
        text(&cli),
        "recv",
        "--source",
        &src,
        "--tmux-socket",
        text(&tm.socket),
        "--state",
        text(&state),
        "--output",
        text(&out),
        "--capture-fps",
        "80",
        "--timeout",
        "35",
    ]);
    wait_file(&secondexit, 40);
    assert_eq!(
        fs::read_to_string(&secondexit).unwrap(),
        "0",
        "{}",
        fs::read_to_string(&secondlog).unwrap()
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&second).unwrap()).unwrap();
    assert!(report["complete"].as_bool().unwrap());
    assert_eq!(fs::read(out.join("源文件.bin")).unwrap(), input);
    let doc = run(&[
        "doctor",
        "--source",
        &src,
        "--tmux-socket",
        text(&tm.socket),
    ]);
    assert!(doc.status.success());
    let doctor: serde_json::Value = serde_json::from_slice(&doc.stdout).unwrap();
    assert_eq!(doctor["pane_fits"], true);
    assert_eq!(doctor["pixel_geometry_verified"], false);
    let status = run(&["status", text(&state)]);
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["finalized"], true);
    let calibrated = run(&[
        "calibrate",
        "--source",
        &src,
        "--tmux-socket",
        text(&tm.socket),
        "--seconds",
        "0.4",
        "--capture-fps",
        "30",
    ]);
    assert!(calibrated.status.success());
    let calibration: serde_json::Value = serde_json::from_slice(&calibrated.stdout).unwrap();
    assert!(calibration["valid_qr_observations"].as_u64().unwrap() > 0);
    // Exercise the actual pause/resume and clean quit keys, including through SSH.
    tm.command(&["send-keys", "-t", &left, "Space"]);
    thread::sleep(Duration::from_millis(200));
    assert!(tm
        .command(&["capture-pane", "-p", "-t", &left])
        .contains("Paused:"));
    tm.command(&["send-keys", "-t", &left, "Space"]);
    thread::sleep(Duration::from_millis(200));
    tm.command(&["send-keys", "-t", &left, "q"]);
    thread::sleep(Duration::from_millis(250));
    assert_eq!(
        tm.command(&["display-message", "-p", "-t", &left, "#{pane_dead}"]),
        "1"
    );
    assert_eq!(
        tm.command(&["display-message", "-p", "-t", &left, "#{alternate_on}"]),
        "0"
    );
    tm.command(&[
        "respawn-pane",
        "-k",
        "-t",
        &left,
        text(&cli),
        "doctor",
        "--show-pattern",
    ]);
    thread::sleep(Duration::from_millis(200));
    let pattern = sources::capture_tmux(&left, Some(&tm.socket), 0)
        .unwrap()
        .unwrap();
    assert!(!sources::decode(&pattern.image).unwrap().is_empty());
    tm.command(&["send-keys", "-t", &left, "q"]);
    thread::sleep(Duration::from_millis(150));
    tm.command(&[
        "respawn-pane",
        "-k",
        "-t",
        &left,
        text(&cli),
        "doctor",
        "--probe-pixels",
    ]);
    thread::sleep(Duration::from_millis(650));
    assert_eq!(
        tm.command(&["display-message", "-p", "-t", &left, "#{pane_dead}"]),
        "1"
    );
    evidence(
        "tmux",
        serde_json::json!({"test":"two local tmux panes; sender in left PTY, receiver in right PTY","sender":std::env::var_os("TQR_ACCEPTANCE_SENDER").map(PathBuf::from).unwrap_or(cli),"playback_prefix":std::env::var("TQR_ACCEPTANCE_PLAYBACK_PREFIX").unwrap_or_else(|_|"[]".into()),"sha256":hex::encode(tqr::protocol::digest(&input)),"source_bytes":input.len(),"first_partial_valid_frames":captured_run,"first_report":first_report,"final_report":report,"resize_pause_verified":true,"sigkill_resume_verified":true,"pause_resume_quit_verified":true,"pattern_and_probe_verified":true,"calibration":calibration,"doctor":doctor,"scope":"Isolated local test environment; any SSH or emulator prefix is recorded above. The user's actual remote host and laptop display were not available."}),
    );
}

fn movie(path: &Path, frames: &[Vec<u8>]) {
    let mut ff = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "gray",
            "-video_size",
            "352x352",
            "-framerate",
            "20",
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = ff.stdin.take().unwrap();
        for bytes in frames {
            let qr = tqr::render::image(bytes, 12, "M", 4).unwrap();
            let mut canvas = image::GrayImage::from_pixel(352, 352, image::Luma([210]));
            image::imageops::overlay(&mut canvas, &qr, 24, 32);
            stdin.write_all(canvas.as_raw()).unwrap();
        }
    }
    let result = ff.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
#[ignore = "requires ffmpeg; validates H.264 recording replay and EOF resume"]
fn h264_video_loss_duplicates_corruption_and_resume() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("recording-source.bin");
    let input: Vec<u8> = (0..17000)
        .map(|i| ((i * 97 + i / 43) % 256) as u8)
        .collect();
    fs::write(&source, &input).unwrap();
    let packed = d.path().join("prepared.tqtx");
    bundle::prepare(&source, &packed, "none", 224).unwrap();
    let mut schedule =
        Schedule::new(Bundle::open(&packed).unwrap(), None, Some(2), Some(19)).unwrap();
    let mut frames = vec![];
    let mut dropped = 0;
    while let Some(f) = schedule.next_frame().unwrap() {
        if f.object == 1 && f.seq % 5 == 0 {
            dropped += 1;
            continue;
        }
        let bytes = f.encode().unwrap();
        frames.push(bytes.clone());
        if f.seq % 7 == 0 {
            frames.push(bytes.clone());
        }
        if f.seq == 4 {
            let mut corrupt = bytes;
            corrupt[100] ^= 0x80;
            frames.push(corrupt);
        }
        if f.seq == 20 {
            let mut foreign = f.clone();
            foreign.transfer = [91; 16];
            frames.push(foreign.encode().unwrap());
        }
    }
    let a = d.path().join("part1.mp4");
    let b = d.path().join("part2.mp4");
    movie(&a, &frames[..70]);
    movie(&b, &frames[70..]);
    // Assert the video adapter emits real, increasing presentation timestamps.
    {
        let mut v = sources::Video::open(&a).unwrap();
        let f = v.next_frame().unwrap().unwrap();
        let g = v.next_frame().unwrap().unwrap();
        assert_eq!(f.pts, Some(0.0));
        assert!(g.pts.unwrap() > 0.0);
    }
    let state = d.path().join("state");
    let output = d.path().join("out");
    let one = run(&[
        "recv",
        "--source",
        &format!("video:{}", a.display()),
        "--state",
        text(&state),
        "--output",
        text(&output),
        "--roi",
        "16,24,320,320",
    ]);
    assert_eq!(
        one.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&one.stderr)
    );
    let first: serde_json::Value = serde_json::from_slice(&one.stdout).unwrap();
    assert!(first["statistics"]["crc_failures"].as_u64().unwrap() > 0);
    assert!(first["statistics"]["duplicate_symbols"].as_u64().unwrap() > 0);
    let two = run(&[
        "recv",
        "--source",
        &format!("video:{}", b.display()),
        "--state",
        text(&state),
        "--output",
        text(&output),
        "--roi",
        "16,24,320,320",
    ]);
    assert!(
        two.status.success(),
        "{}",
        String::from_utf8_lossy(&two.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&two.stdout).unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(
        fs::read(output.join("recording-source.bin")).unwrap(),
        input
    );
    evidence(
        "video",
        serde_json::json!({"test":"H.264 video, deterministic 20% data-frame erasure, duplicates, a CRC-invalid QR, mixed transfer, two recordings into one state directory","dropped_display_frames":dropped,"sha256":hex::encode(tqr::protocol::digest(&input)),"first_report":first,"final_report":report}),
    );
}
