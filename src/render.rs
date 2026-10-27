use crate::protocol::Frame;
use anyhow::{ensure, Result};
use image::{GrayImage, Luma};
use qrcodegen::{QrCode, QrCodeEcc, QrSegment, Version};

pub fn ecc(level: &str) -> Result<QrCodeEcc> {
    Ok(match level {
        "L" => QrCodeEcc::Low,
        "M" => QrCodeEcc::Medium,
        "Q" => QrCodeEcc::Quartile,
        "H" => QrCodeEcc::High,
        _ => anyhow::bail!("ECC must be L, M, Q or H"),
    })
}

pub fn qr(bytes: &[u8], version: u8, level: &str) -> Result<QrCode> {
    ensure!((1..=40).contains(&version), "QR version must be 1..=40");
    Ok(QrCode::encode_segments_advanced(
        &[QrSegment::make_bytes(bytes)],
        ecc(level)?,
        Version::new(version),
        Version::new(version),
        None,
        false,
    )?)
}

pub fn geometry(version: u8) -> Result<(u16, u16)> {
    ensure!((1..=40).contains(&version), "QR version must be 1..=40");
    let q = 4 * u16::from(version) + 25;
    Ok((q, q.div_ceil(2)))
}

pub fn fits(version: u8, cols: u16, rows: u16) -> bool {
    geometry(version).is_ok_and(|(w, h)| w.max(55) < cols && h + 3 <= rows)
}

pub fn check_capacity(size: u16, version: u8, level: &str) -> Result<()> {
    qr(
        &vec![0; usize::from(size) + crate::protocol::OVERHEAD],
        version,
        level,
    )?;
    Ok(())
}

fn dark(code: &QrCode, x: i32, y: i32) -> bool {
    code.get_module(x - 4, y - 4)
}

pub fn terminal(frame: &Frame, version: u8, level: &str, round: u64) -> Result<String> {
    let bytes = frame.encode()?;
    let code = qr(&bytes, version, level)?;
    let q = code.size() + 8;
    let marker = frame.marker(&bytes);
    // Guard first, matching footer last. Every cell, including white quiet zones,
    // has explicit colours; terminal theme and trailing-space trimming cannot alter it.
    let mut out = format!("\x1b[H\x1b[0m\x1b[2K{marker}\r\n");
    for y in (0..q).step_by(2) {
        out.push_str("\x1b[38;2;0;0;0m\x1b[48;2;255;255;255m");
        for x in 0..q {
            let a = dark(&code, x, y);
            let b = y + 1 < q && dark(&code, x, y + 1);
            out.push(match (a, b) {
                (false, false) => ' ',
                (true, false) => '▀',
                (false, true) => '▄',
                (true, true) => '█',
            });
        }
        out.push_str("\x1b[0m\x1b[K\r\n");
    }
    // The marker is at the start of the footer; diagnostics follow only if they fit.
    let diagnostic = format!(
        "round={round} g={} s={} | q=quit space=pause",
        frame.generation, frame.symbol
    );
    let diagnostic: String = diagnostic.chars().take(q.max(55) as usize).collect();
    out.push_str(&format!("\x1b[0m\x1b[2K{marker}\r\n\x1b[2K{diagnostic}"));
    Ok(out)
}

pub fn image(bytes: &[u8], version: u8, level: &str, scale: u32) -> Result<GrayImage> {
    ensure!((1..=32).contains(&scale), "invalid image scale");
    let code = qr(bytes, version, level)?;
    let q = (code.size() + 8) as u32;
    Ok(GrayImage::from_fn(q * scale, q * scale, |x, y| {
        Luma([if dark(&code, (x / scale) as i32, (y / scale) as i32) {
            0
        } else {
            255
        }])
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity_and_geometry() {
        assert_eq!(geometry(12).unwrap(), (73, 37));
        assert!(fits(12, 120, 40));
        assert!(!fits(12, 120, 39));
        assert!(!fits(12, 73, 40));
        for (v, s) in [(12, 224), (17, 440), (22, 712)] {
            check_capacity(s, v, "M").unwrap();
        }
        assert!(check_capacity(240, 12, "M").is_err());
        assert!(qr(&[0; 287], 12, "M").is_ok());
        assert!(qr(&[0; 288], 12, "M").is_err());
    }
}
