//! Export real protocol frames for the deterministic HTML film.
use anyhow::{Context, Result};
use shadow_ink::{bundle, protocol::Frame, render, sender::Schedule, text};
use std::{env, fs, path::Path};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    anyhow::ensure!(args.len() == 3, "usage: demo_frames SOURCE OUTPUT.js");
    let source = Path::new(&args[1]);
    let temp = tempfile::tempdir()?;
    let mut modes = Vec::new();
    let mut original = None;
    for (name, size, version, ecc) in [
        ("qr12", 224, Some(12), "M"),
        ("qr40", 2893, Some(40), "L"),
        ("pipe", 2893, None, "L"),
    ] {
        let path = temp.path().join(format!("{name}.tqtx"));
        let manifest = bundle::prepare(source, &path, "none", size)?;
        original = Some(manifest.clone());
        let mut schedule = Schedule::new(bundle::Bundle::open(&path)?, None, Some(1), Some(2026))?;
        let mut frames = Vec::new();
        // Include metadata and systematically encoded data frames.
        while frames.len() < 32 {
            let Some(frame) = schedule.next_frame()? else {
                break;
            };
            let encoded = frame.encode()?;
            anyhow::ensure!(Frame::decode(&encoded)? == frame, "frame round trip failed");
            let rows = if let Some(version) = version {
                let qr = render::qr(&encoded, version, ecc)?;
                Some(
                    (0..qr.size())
                        .map(|y| {
                            (0..qr.size())
                                .map(|x| if qr.get_module(x, y) { '1' } else { '0' })
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            frames.push(serde_json::json!({"rows":rows,"line":text::line(&frame)?,"sequence":frame.seq,"object":frame.object,"generation":frame.generation,"symbol":frame.symbol}));
        }
        modes.push((
            name.to_owned(),
            serde_json::json!({"symbolBytes":size,"version":version,"ecc":ecc,"frames":frames}),
        ));
    }
    let manifest = original.context("no manifest")?;
    let data = serde_json::json!({"filename":manifest.filename,"bytes":manifest.original_bytes,"sha256":manifest.original_sha256,"modes":modes.into_iter().collect::<serde_json::Map<_,_>>()});
    fs::write(
        &args[2],
        format!(
            "window.SHADOW_INK_PAYLOAD = {};\n",
            serde_json::to_string(&data)?
        ),
    )?;
    println!(
        "Exported real frames for {} ({} bytes)",
        source.display(),
        manifest.original_bytes
    );
    Ok(())
}
