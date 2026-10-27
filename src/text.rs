//! ASCII transport envelope around the unchanged CRC-protected binary frame.
use crate::protocol::{Frame, MAX_SYMBOL, OVERHEAD};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD, Engine};

pub const PREFIX: &str = "SI1 ";
pub const MAX_LINE: usize = 4 + (MAX_SYMBOL + OVERHEAD).div_ceil(3) * 4;

pub fn line(frame: &Frame) -> Result<String> {
    Ok(format!("{PREFIX}{}", STANDARD.encode(frame.encode()?)))
}

/// Noise, ANSI cursor prefixes and unrelated shell output cannot become frames.
/// The caller still checks the binary frame CRC and all protocol limits.
pub fn decode_line(line: &[u8]) -> Option<Result<Vec<u8>>> {
    let start = line
        .windows(PREFIX.len())
        .rposition(|w| w == PREFIX.as_bytes())?;
    let encoded = line[start + PREFIX.len()..]
        .strip_suffix(b"\r")
        .unwrap_or(&line[start + PREFIX.len()..]);
    if encoded.len() > MAX_LINE - PREFIX.len() {
        return Some(Err(anyhow::anyhow!("text line exceeds limit")));
    }
    Some(STANDARD.decode(encoded).map_err(Into::into))
}

/// Bounded incremental parsing: partial lines survive reads; overlong lines are
/// discarded through the next newline, allowing the following packet to recover.
#[derive(Default)]
pub struct Lines {
    pending: Vec<u8>,
    overflow: bool,
}
impl Lines {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<Vec<u8>>> {
        let mut packets = Vec::new();
        for &b in bytes {
            if b == b'\n' {
                if !self.overflow {
                    if let Some(packet) = decode_line(&self.pending) {
                        packets.push(packet);
                    }
                }
                self.pending.clear();
                self.overflow = false;
            } else if !self.overflow {
                if self.pending.len() >= MAX_LINE + 64 {
                    self.pending.clear();
                    self.overflow = true;
                } else {
                    self.pending.push(b);
                }
            }
        }
        packets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_noise_damage_and_resync() {
        let frame = crate::protocol::tests::frame();
        let line = line(&frame).unwrap();
        let mut parser = Lines::default();
        assert!(parser.push(b"shell prompt\r\n\x1b[H").is_empty());
        assert!(parser.push(&line.as_bytes()[..30]).is_empty());
        let mut rest = line.as_bytes()[30..].to_vec();
        rest.extend_from_slice(b"\r\n");
        let packets = parser.push(&rest);
        assert_eq!(Frame::decode(packets[0].as_ref().unwrap()).unwrap(), frame);
        let mut damaged = line.into_bytes();
        damaged[20] = if damaged[20] == b'A' { b'B' } else { b'A' };
        damaged.push(b'\n');
        assert!(Frame::decode(parser.push(&damaged)[0].as_ref().unwrap()).is_err());
        assert!(parser.push(&vec![b'x'; MAX_LINE * 4]).is_empty());
        assert!(parser.push(b"\n").is_empty());
        assert_eq!(
            parser
                .push(format!("{}\n", super::line(&frame).unwrap()).as_bytes())
                .len(),
            1
        );
    }
}
