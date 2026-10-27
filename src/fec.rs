use crate::protocol::digest;
use anyhow::{ensure, Result};
use reed_solomon_erasure::galois_8::ReedSolomon;

pub const GROUP_OVERHEAD: usize = 36;

pub fn capacity(k: u8, size: usize) -> usize {
    usize::from(k) * size - GROUP_OVERHEAD
}

pub fn encode(data: &[u8], k: u8, n: u8, size: usize) -> Result<Vec<Vec<u8>>> {
    ensure!(
        k > 0 && k < n && n <= 96 && size >= 64,
        "invalid group parameters"
    );
    ensure!(
        data.len() <= capacity(k, size),
        "generation data exceeds capacity"
    );
    let mut raw = vec![0u8; usize::from(k) * size];
    raw[..4].copy_from_slice(&(data.len() as u32).to_be_bytes());
    raw[4..36].copy_from_slice(&digest(data));
    raw[36..36 + data.len()].copy_from_slice(data);
    let mut shards: Vec<Vec<u8>> = raw.chunks_exact(size).map(|s| s.to_vec()).collect();
    shards.extend((k..n).map(|_| vec![0u8; size]));
    ReedSolomon::new(k as usize, (n - k) as usize)?.encode(&mut shards)?;
    Ok(shards)
}

pub fn decode(mut shards: Vec<Option<Vec<u8>>>, k: u8, n: u8, size: usize) -> Result<Vec<u8>> {
    ensure!(
        k > 0 && k < n && n <= 96 && shards.len() == usize::from(n),
        "invalid group parameters"
    );
    ensure!(
        shards.iter().flatten().all(|s| s.len() == size),
        "inconsistent shard size"
    );
    ReedSolomon::new(k as usize, (n - k) as usize)?.reconstruct_data(&mut shards)?;
    let raw: Vec<u8> = shards
        .into_iter()
        .take(k as usize)
        .flatten()
        .flatten()
        .collect();
    ensure!(raw.len() >= GROUP_OVERHEAD, "short recovered group");
    let len = u32::from_be_bytes(raw[..4].try_into()?) as usize;
    ensure!(len <= capacity(k, size), "invalid recovered length");
    let data = &raw[36..36 + len];
    ensure!(
        digest(data).as_slice() == &raw[4..36],
        "generation SHA-256 mismatch"
    );
    ensure!(
        raw[36 + len..].iter().all(|&v| v == 0),
        "nonzero generation padding"
    );
    Ok(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{seq::SliceRandom, SeedableRng};
    #[test]
    fn loss_bursts_and_shuffle() {
        let input: Vec<_> = (0..capacity(64, 224)).map(|n| (n * 31) as u8).collect();
        let all = encode(&input, 64, 96, 224).unwrap();
        let mut rng = rand::rngs::StdRng::seed_from_u64(19);
        for _ in 0..32 {
            let mut ids: Vec<_> = (0..96).collect();
            ids.shuffle(&mut rng);
            let mut parts = vec![None; 96];
            for &i in &ids[..64] {
                parts[i] = Some(all[i].clone());
            }
            assert_eq!(decode(parts, 64, 96, 224).unwrap(), input);
        }
        let mut parts: Vec<_> = all.iter().cloned().map(Some).collect();
        for s in &mut parts[20..52] {
            *s = None;
        }
        assert_eq!(decode(parts, 64, 96, 224).unwrap(), input);
        let mut parts: Vec<_> = all.into_iter().map(Some).collect();
        for s in &mut parts[..33] {
            *s = None;
        }
        assert!(decode(parts, 64, 96, 224).is_err());
    }
    #[test]
    fn hashes_and_empty() {
        let mut a = encode(b"hello", 2, 3, 64).unwrap();
        a[0][40] ^= 1;
        assert!(decode(a.into_iter().map(Some).collect(), 2, 3, 64).is_err());
        let a = encode(&[], 1, 2, 64).unwrap();
        assert_eq!(
            decode(vec![None, Some(a[1].clone())], 1, 2, 64).unwrap(),
            b""
        );
    }
}
