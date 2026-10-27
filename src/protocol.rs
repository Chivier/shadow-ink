use anyhow::{bail, ensure, Result};
use sha2::{Digest, Sha256};

pub const HEADER_LEN: usize = 56;
pub const OVERHEAD: usize = 60;
pub const MAGIC: &[u8; 4] = b"TQR\0";
pub const MAX_SYMBOL: usize = 2893; // QR V40-L byte capacity minus our header/CRC
pub const MAX_MANIFEST: usize = 8192;
pub const DATA_K: u8 = 64;
pub const DATA_N: u8 = 96;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub transfer: [u8; 16],
    pub object: u32,
    pub generation: u32,
    pub symbol: u16,
    pub k: u8,
    pub n: u8,
    pub symbol_bytes: u16,
    pub run: u64,
    pub seq: u64,
    pub payload: Vec<u8>,
}

pub fn digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub fn transfer_id(manifest: &[u8]) -> [u8; 16] {
    digest(manifest)[..16].try_into().unwrap()
}

impl Frame {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.object <= 1, "unsupported object ID");
        ensure!(
            self.k > 0 && self.k <= DATA_K && self.k < self.n && self.n <= DATA_N,
            "invalid FEC parameters"
        );
        ensure!(self.symbol < u16::from(self.n), "symbol out of range");
        ensure!(
            (64..=MAX_SYMBOL).contains(&usize::from(self.symbol_bytes)),
            "invalid symbol size"
        );
        ensure!(
            self.payload.len() == usize::from(self.symbol_bytes),
            "payload size mismatch"
        );
        if self.object == 0 {
            ensure!(self.generation == 0, "manifest generation must be zero");
        } else {
            ensure!(
                self.k == DATA_K && self.n == DATA_N,
                "unsupported data FEC profile"
            );
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut out = Vec::with_capacity(OVERHEAD + self.payload.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&[1, self.object as u8, 1, 0]); // version, type, GF256 profile, flags
        out.extend_from_slice(&self.transfer);
        out.extend_from_slice(&self.object.to_be_bytes());
        out.extend_from_slice(&self.generation.to_be_bytes());
        out.extend_from_slice(&self.symbol.to_be_bytes());
        out.extend_from_slice(&[self.k, self.n]);
        out.extend_from_slice(&self.symbol_bytes.to_be_bytes());
        out.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        out.extend_from_slice(&self.run.to_be_bytes());
        out.extend_from_slice(&self.seq.to_be_bytes());
        out.extend_from_slice(&self.payload);
        out.extend_from_slice(&crc32c::crc32c(&out).to_be_bytes());
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() >= OVERHEAD && bytes.len() <= OVERHEAD + MAX_SYMBOL,
            "frame length out of bounds"
        );
        ensure!(&bytes[..4] == MAGIC, "not a tqr frame");
        ensure!(
            bytes[4] == 1 && bytes[6] == 1 && bytes[7] == 0,
            "unsupported protocol/profile/flags"
        );
        let u16at = |i| u16::from_be_bytes(bytes[i..i + 2].try_into().unwrap());
        let u32at = |i| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
        let payload_len = usize::from(u16at(38));
        ensure!(
            bytes.len() == OVERHEAD + payload_len,
            "declared frame length mismatch"
        );
        let end = bytes.len() - 4;
        ensure!(
            crc32c::crc32c(&bytes[..end]) == u32at(end),
            "CRC32C mismatch"
        );
        let f = Self {
            transfer: bytes[8..24].try_into().unwrap(),
            object: u32at(24),
            generation: u32at(28),
            symbol: u16at(32),
            k: bytes[34],
            n: bytes[35],
            symbol_bytes: u16at(36),
            run: u64::from_be_bytes(bytes[40..48].try_into().unwrap()),
            seq: u64::from_be_bytes(bytes[48..56].try_into().unwrap()),
            payload: bytes[HEADER_LEN..end].to_vec(),
        };
        ensure!(
            u32::from(bytes[5]) == f.object,
            "frame type/object mismatch"
        );
        f.validate()?;
        Ok(f)
    }

    pub fn marker(&self, encoded: &[u8]) -> String {
        format!(
            "TQ1 {} {:016x} {:016x} {}",
            hex::encode(&self.transfer[..4]),
            self.run,
            self.seq,
            hex::encode(&encoded[encoded.len() - 4..])
        )
    }
}

pub fn parse_generations(spec: &str, total: u32) -> Result<Vec<u32>> {
    let mut out = std::collections::BTreeSet::new();
    for item in spec.split(',') {
        let (a, b) = match item.split_once('-') {
            Some((a, b)) => (a.parse::<u32>()?, b.parse::<u32>()?),
            None => {
                let n = item.parse::<u32>()?;
                (n, n)
            }
        };
        ensure!(a <= b && b < total, "generation range outside 0..{total}");
        // Avoid expanding an unbounded user range before validation.
        if b - a > 10_000_000 {
            bail!("generation range too large");
        }
        out.extend(a..=b);
    }
    ensure!(!out.is_empty(), "no generations selected");
    Ok(out.into_iter().collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub fn frame() -> Frame {
        Frame {
            transfer: [42; 16],
            object: 1,
            generation: 7,
            symbol: 3,
            k: 64,
            n: 96,
            symbol_bytes: 224,
            run: 123,
            seq: 456,
            payload: (0..224).map(|n| n as u8).collect(),
        }
    }
    #[test]
    fn wire_and_damage() {
        assert_eq!(crc32c::crc32c(b"123456789"), 0xe3069283);
        let f = frame();
        let bytes = f.encode().unwrap();
        assert_eq!(bytes.len(), 284);
        assert_eq!(&bytes[40..48], &123u64.to_be_bytes());
        assert_eq!(Frame::decode(&bytes).unwrap(), f);
        for i in 0..bytes.len() {
            let mut bad = bytes.clone();
            bad[i] ^= 1;
            assert!(Frame::decode(&bad).is_err(), "offset {i}");
        }
        for n in 0..bytes.len() {
            assert!(Frame::decode(&bytes[..n]).is_err());
        }
    }
    #[test]
    fn generation_ranges() {
        assert_eq!(
            parse_generations("3,8-10,3", 11).unwrap(),
            vec![3, 8, 9, 10]
        );
        assert!(parse_generations("0-4294967295", 10).is_err());
    }
}
