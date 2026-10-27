use sha2::{Digest, Sha256};

#[test]
fn matches_independent_python_frame_vector() {
    let v: serde_json::Value = serde_json::from_str(include_str!("vectors/frame-v1.json")).unwrap();
    let bytes = hex::decode(v["frame_hex"].as_str().unwrap()).unwrap();
    let f = shadow_ink::protocol::Frame::decode(&bytes).unwrap();
    assert_eq!(
        (f.run, f.seq, f.object, f.generation, f.symbol),
        (123, 456, 1, 7, 3)
    );
    assert_eq!(f.payload, (0..224).collect::<Vec<u8>>());
    assert_eq!(f.encode().unwrap(), bytes);
    assert_eq!(
        hex::encode(Sha256::digest(&bytes)),
        v["frame_sha256"].as_str().unwrap()
    );
}

#[test]
fn matches_independent_python_wire_vectors() {
    for raw in [
        include_str!("vectors/rs-64-96-224.json"),
        include_str!("vectors/rs-3-5-224.json"),
    ] {
        let v: serde_json::Value = serde_json::from_str(raw).unwrap();
        let k = v["k"].as_u64().unwrap() as u8;
        let n = v["n"].as_u64().unwrap() as u8;
        let size = v["symbol_bytes"].as_u64().unwrap() as usize;
        let input: Vec<_> = (0..shadow_ink::fec::capacity(k, size))
            .map(|i| (i * 31) as u8)
            .collect();
        let shards = shadow_ink::fec::encode(&input, k, n, size).unwrap();
        let mut total = Sha256::new();
        for (s, hash) in shards.iter().zip(v["shard_sha256"].as_array().unwrap()) {
            assert_eq!(hex::encode(Sha256::digest(s)), hash.as_str().unwrap());
            total.update(s);
        }
        assert_eq!(
            hex::encode(total.finalize()),
            v["all_shards_sha256"].as_str().unwrap()
        );
    }
}
