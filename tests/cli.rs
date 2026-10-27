use std::{fs, process::Command};
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tqr"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn help_doctor_and_invalid_arguments() {
    let help = cli(&["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("prepare"));
    assert_eq!(help.contains("recv"), cfg!(feature = "receiver"));
    let d = cli(&["doctor", "--cell-pixels", "8x16"]);
    assert!(d.status.success());
    let v: serde_json::Value = serde_json::from_slice(&d.stdout).unwrap();
    assert_eq!(v["module_width_height_ratio"], 1.0);
    assert_eq!(v["cell_pixels_source"], "explicit");
    assert_eq!(v["pixel_geometry_verified"], false);
    assert_eq!(v["packet_bytes"], 284);
    for args in [
        vec!["doctor", "--cell-pixels", "8x0"],
        vec!["doctor", "--qr-version", "0"],
        vec!["doctor", "--ecc", "X"],
    ] {
        assert!(!cli(&args).status.success());
    }
}

#[test]
fn prepare_inspect_and_refuse_non_tty_send() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("source.txt");
    let out = d.path().join("source.tqtx");
    fs::write(&src, b"hello terminal").unwrap();
    let p = cli(&[
        "prepare",
        src.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    let v: serde_json::Value = serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["manifest"]["original_bytes"], 14);
    assert_eq!(v["transfer_id"].as_str().unwrap().len(), 32);
    let before = fs::read(&out).unwrap();
    assert!(!cli(&[
        "prepare",
        src.to_str().unwrap(),
        "--out",
        out.to_str().unwrap()
    ])
    .status
    .success());
    assert_eq!(fs::read(&out).unwrap(), before);
    let i = cli(&["inspect", out.to_str().unwrap()]);
    assert!(i.status.success());
    let inspect: serde_json::Value = serde_json::from_slice(&i.stdout).unwrap();
    assert_eq!(inspect["transfer_id"], v["transfer_id"]);
    let send = cli(&["send", out.to_str().unwrap(), "--rounds", "1"]);
    assert!(!send.status.success());
    assert!(String::from_utf8(send.stderr)
        .unwrap()
        .contains("requires a real terminal"));
}
