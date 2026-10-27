use crate::{
    fec,
    protocol::{self, DATA_K, DATA_N, MAX_MANIFEST, MAX_SYMBOL},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use tempfile::NamedTempFile;

const BUNDLE_MAGIC: &[u8; 8] = b"TQTX0001";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u8,
    pub filename: String,
    pub original_bytes: u64,
    pub original_sha256: String,
    pub transport_bytes: u64,
    pub transport_sha256: String,
    pub compression: String,
    pub symbol_bytes: u16,
    pub k: u8,
    pub n: u8,
}

pub fn safe_filename(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 1024
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        && !name.ends_with(['.', ' '])
}

impl Manifest {
    pub fn parse(raw: &[u8]) -> Result<Self> {
        ensure!(raw.len() <= MAX_MANIFEST, "manifest too large");
        let m: Self = serde_json::from_slice(raw).context("invalid manifest JSON")?;
        m.validate()?;
        Ok(m)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(self.version == 1, "unsupported manifest version");
        ensure!(safe_filename(&self.filename), "unsafe output filename");
        ensure!(
            (64..=MAX_SYMBOL).contains(&(self.symbol_bytes as usize)),
            "invalid symbol size"
        );
        ensure!(
            self.k == DATA_K && self.n == DATA_N,
            "unsupported FEC profile"
        );
        ensure!(
            matches!(self.compression.as_str(), "none" | "zstd"),
            "unsupported compression"
        );
        for h in [&self.original_sha256, &self.transport_sha256] {
            ensure!(
                h.len() == 64
                    && h.bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "invalid SHA-256"
            );
        }
        if self.compression == "none" {
            ensure!(
                self.original_bytes == self.transport_bytes
                    && self.original_sha256 == self.transport_sha256,
                "uncompressed manifest inconsistency"
            );
        }
        ensure!(
            self.generations_u64() <= u64::from(u32::MAX),
            "too many generations"
        );
        Ok(())
    }
    pub fn group_capacity(&self) -> usize {
        fec::capacity(self.k, self.symbol_bytes as usize)
    }
    fn generations_u64(&self) -> u64 {
        self.transport_bytes
            .div_ceil(self.group_capacity() as u64)
            .max(1)
    }
    pub fn generations(&self) -> u32 {
        self.generations_u64() as u32
    }
    pub fn generation_len(&self, g: u32) -> Result<usize> {
        ensure!(g < self.generations(), "generation out of range");
        Ok(self
            .transport_bytes
            .saturating_sub(u64::from(g) * self.group_capacity() as u64)
            .min(self.group_capacity() as u64) as usize)
    }
}

pub fn hash_reader(mut reader: impl Read) -> Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0u8; 65536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .context("file length overflow")?;
        hash.update(&buf[..n]);
    }
    Ok((count, hex::encode(hash.finalize())))
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

pub fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    Ok(())
}

pub fn prepare(
    input: &Path,
    output: &Path,
    compression: &str,
    symbol_bytes: u16,
) -> Result<Manifest> {
    ensure!(
        matches!(compression, "none" | "auto" | "zstd"),
        "compression must be none, auto or zstd"
    );
    ensure!(
        (64..=MAX_SYMBOL).contains(&(symbol_bytes as usize)),
        "symbol bytes must be 64..={MAX_SYMBOL}"
    );
    ensure!(
        !output.exists(),
        "output already exists: {}",
        output.display()
    );
    let mut src = File::open(input).context("open source file")?;
    let before = src.metadata()?;
    ensure!(before.is_file(), "source must be a regular file");
    let filename = input
        .file_name()
        .context("missing filename")?
        .to_str()
        .context("filename must be UTF-8")?
        .to_owned();
    ensure!(
        safe_filename(&filename),
        "filename is not a safe portable basename"
    );
    fs::create_dir_all(parent(output))?;
    // Snapshot once; compression and later playback never reread a changing source.
    let mut snapshot = NamedTempFile::new_in(parent(output))?;
    std::io::copy(&mut src, &mut snapshot)?;
    let after = src.metadata()?;
    ensure!(
        before.len() == after.len()
            && before.modified()? == after.modified()?
            && snapshot.as_file().metadata()?.len() == before.len(),
        "source changed while preparing; take a consistent snapshot first"
    );
    snapshot.as_file_mut().seek(SeekFrom::Start(0))?;
    let (original_bytes, original_sha256) = hash_reader(snapshot.as_file_mut())?;
    let mut packed = NamedTempFile::new_in(parent(output))?;
    let mut chosen = "none";
    if compression != "none" {
        snapshot.as_file_mut().seek(SeekFrom::Start(0))?;
        zstd::stream::copy_encode(snapshot.as_file_mut(), &mut packed, 3)?;
        if compression == "zstd" || packed.as_file().metadata()?.len() < original_bytes {
            chosen = "zstd";
        }
    }
    let transport = if chosen == "zstd" {
        packed.as_file_mut()
    } else {
        snapshot.as_file_mut()
    };
    transport.seek(SeekFrom::Start(0))?;
    let (transport_bytes, transport_sha256) = hash_reader(&mut *transport)?;
    let m = Manifest {
        version: 1,
        filename,
        original_bytes,
        original_sha256,
        transport_bytes,
        transport_sha256,
        compression: chosen.into(),
        symbol_bytes,
        k: DATA_K,
        n: DATA_N,
    };
    m.validate()?;
    let raw = serde_json::to_vec(&m)?;
    ensure!(raw.len() <= MAX_MANIFEST, "manifest too large");
    manifest_fec(&raw, symbol_bytes)?;
    let mut bundle = NamedTempFile::new_in(parent(output))?;
    bundle.write_all(BUNDLE_MAGIC)?;
    bundle.write_all(&(raw.len() as u32).to_be_bytes())?;
    bundle.write_all(&raw)?;
    transport.seek(SeekFrom::Start(0))?;
    std::io::copy(transport, &mut bundle)?;
    bundle.as_file().sync_all()?;
    bundle
        .persist_noclobber(output)
        .map_err(|e| e.error)
        .context("publish prepared bundle without overwriting")?;
    sync_dir(parent(output))?;
    Ok(m)
}

pub fn manifest_fec(raw: &[u8], size: u16) -> Result<(u8, u8, Vec<Vec<u8>>)> {
    ensure!(
        (64..=MAX_SYMBOL).contains(&(size as usize)),
        "invalid manifest symbol size"
    );
    ensure!(raw.len() <= MAX_MANIFEST, "manifest too large");
    let k = (raw.len() + fec::GROUP_OVERHEAD)
        .div_ceil(size as usize)
        .max(1);
    let n = k + k.div_ceil(2);
    ensure!(
        k <= 64 && n <= 96,
        "manifest does not fit; use larger symbol size"
    );
    Ok((
        k as u8,
        n as u8,
        fec::encode(raw, k as u8, n as u8, size as usize)?,
    ))
}

pub struct Bundle {
    pub manifest: Manifest,
    pub raw: Vec<u8>,
    pub id: [u8; 16],
    file: File,
    offset: u64,
}

impl Bundle {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut head = [0u8; 12];
        file.read_exact(&mut head)?;
        ensure!(&head[..8] == BUNDLE_MAGIC, "not a tqr prepared bundle");
        let len = u32::from_be_bytes(head[8..12].try_into()?) as usize;
        ensure!(
            len > 0 && len <= MAX_MANIFEST,
            "manifest length out of bounds"
        );
        let mut raw = vec![0u8; len];
        file.read_exact(&mut raw)?;
        let manifest = Manifest::parse(&raw)?;
        let offset = 12 + len as u64;
        ensure!(
            file.metadata()?.len()
                == offset
                    .checked_add(manifest.transport_bytes)
                    .context("bundle length overflow")?,
            "bundle length mismatch"
        );
        let (bytes, sha) = hash_reader(&mut file)?;
        ensure!(
            bytes == manifest.transport_bytes && sha == manifest.transport_sha256,
            "prepared bundle SHA-256 mismatch"
        );
        manifest_fec(&raw, manifest.symbol_bytes)?;
        Ok(Self {
            id: protocol::transfer_id(&raw),
            raw,
            manifest,
            file,
            offset,
        })
    }
    pub fn shards(&mut self, g: u32) -> Result<Vec<Vec<u8>>> {
        let len = self.manifest.generation_len(g)?;
        let start = self.offset + u64::from(g) * self.manifest.group_capacity() as u64;
        self.file.seek(SeekFrom::Start(start))?;
        let mut data = vec![0u8; len];
        self.file.read_exact(&mut data)?;
        fec::encode(&data, DATA_K, DATA_N, self.manifest.symbol_bytes as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepare_modes_and_tampering() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("中文.bin");
        fs::write(&p, vec![9u8; 50000]).unwrap();
        for mode in ["none", "auto", "zstd"] {
            let out = d.path().join(format!("{mode}.tqtx"));
            let m = prepare(&p, &out, mode, 224).unwrap();
            let mut b = Bundle::open(&out).unwrap();
            assert_eq!(m.original_bytes, 50000);
            assert_eq!(b.id, protocol::transfer_id(&b.raw));
            assert_eq!(b.shards(0).unwrap().len(), 96);
            assert!(prepare(&p, &out, mode, 224).is_err());
        }
        let out = d.path().join("none.tqtx");
        let mut b = fs::read(&out).unwrap();
        *b.last_mut().unwrap() ^= 1;
        fs::write(&out, b).unwrap();
        assert!(Bundle::open(&out).is_err());
    }
    #[test]
    fn unsafe_names() {
        for n in ["../x", "/x", "a\\b", "..", "a\n", "x:", "x."] {
            assert!(!safe_filename(n));
        }
        assert!(safe_filename("中文 空格.txt"));
    }
}
