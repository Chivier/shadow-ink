#![cfg(feature = "receiver")]
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
};
use tqr::{
    bundle::{self, Bundle},
    protocol::{transfer_id, Frame},
    sender::Schedule,
    store::{Ingest, Store},
};

fn setup(input: &[u8], compression: &str) -> (tempfile::TempDir, std::path::PathBuf, Vec<Frame>) {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("实验结果.bin");
    fs::write(&src, input).unwrap();
    let path = d.path().join("bundle.tqtx");
    bundle::prepare(&src, &path, compression, 224).unwrap();
    let mut schedule =
        Schedule::new(Bundle::open(&path).unwrap(), None, Some(1), Some(42)).unwrap();
    let mut frames = vec![];
    while let Some(f) = schedule.next_frame().unwrap() {
        frames.push(f);
    }
    (d, path, frames)
}

#[test]
fn resume_late_manifest_duplicates_noise_and_export() {
    let input: Vec<u8> = (0..42000).map(|i| (i * 37) as u8).collect();
    let (d, _, frames) = setup(&input, "none");
    let state = d.path().join("state");
    let output = d.path().join("out");
    let data: Vec<_> = frames
        .iter()
        .filter(|f| f.object == 1 && f.symbol < 64)
        .collect();
    {
        let mut s = Store::open(&state, 1 << 30, None).unwrap();
        // Start late: data arrives before the repeated manifest, then stop halfway.
        for f in data.iter().take(90) {
            s.ingest(f).unwrap();
            assert_eq!(s.ingest(f).unwrap(), Ingest::Duplicate);
        }
        assert!(s.active.is_none());
    }
    {
        let mut s = Store::open(&state, 1 << 30, None).unwrap();
        for f in frames.iter().filter(|f| f.object == 0) {
            s.ingest(f).unwrap();
        }
        for f in data.iter().skip(90) {
            s.ingest(f).unwrap();
        }
        assert!(s.ready().unwrap());
        assert_eq!(s.status().unwrap().completed_generations, 3);
        let file = s.finalize(&output).unwrap();
        assert_eq!(fs::read(file).unwrap(), input);
        let mut foreign = frames[0].clone();
        foreign.transfer = [55; 16];
        assert_eq!(s.ingest(&foreign).unwrap(), Ingest::Ignored);
        assert!(
            Store::open(&state, 1 << 30, None).is_err(),
            "concurrent receivers must be excluded"
        );
    }
    let mut s = Store::open(&state, 1 << 30, None).unwrap();
    assert!(s.ready().unwrap());
    s.finalize(&output).unwrap();
    fs::write(output.join("实验结果.bin"), b"keep this existing file").unwrap();
    assert!(
        !s.status().unwrap().finalized,
        "status must detect changed output"
    );
    assert!(s.finalize(&output).is_err());
    assert_eq!(
        fs::read(output.join("实验结果.bin")).unwrap(),
        b"keep this existing file"
    );
    fs::remove_file(output.join("实验结果.bin")).unwrap();
    assert!(!s.status().unwrap().finalized);
    let restored = s.finalize(&output).unwrap();
    assert_eq!(fs::read(restored).unwrap(), input);
}

#[test]
fn corrupted_part_and_cached_shard_are_recovered() {
    let (d, _, frames) = setup(&vec![8; 25000], "none");
    let state = d.path().join("state");
    let output = d.path().join("out");
    let id = frames[0].transfer;
    {
        let mut s = Store::open(&state, 1 << 30, None).unwrap();
        for f in frames.iter().filter(|f| f.object == 0 || f.generation == 0) {
            s.ingest(f).unwrap();
        }
        assert_eq!(s.status().unwrap().completed_generations, 1);
        let f = frames
            .iter()
            .find(|f| f.object == 1 && f.generation == 1)
            .unwrap();
        s.ingest(f).unwrap();
        s.conn
            .execute(
                "UPDATE shards SET payload=zeroblob(length(payload)) WHERE obj=1",
                [],
            )
            .unwrap();
        let mut part = fs::OpenOptions::new()
            .write(true)
            .open(s.part_path(&id))
            .unwrap();
        part.seek(SeekFrom::Start(10)).unwrap();
        part.write_all(b"bad").unwrap();
        part.sync_all().unwrap();
    }
    let mut s = Store::open(&state, 1 << 30, None).unwrap();
    assert_eq!(s.status().unwrap().completed_generations, 0);
    for f in frames {
        s.ingest(&f).unwrap();
    }
    assert!(s.ready().unwrap());
    let file = s.finalize(&output).unwrap();
    assert_eq!(fs::read(file).unwrap(), vec![8; 25000]);
}

#[test]
fn empty_and_zstd_files() {
    for input in [
        vec![],
        vec![0; 50000],
        (0..1024).map(|i| (i * 97) as u8).collect(),
    ] {
        let (d, _, frames) = setup(&input, "zstd");
        let mut s = Store::open(&d.path().join("state"), 1 << 30, None).unwrap();
        for f in frames {
            s.ingest(&f).unwrap();
        }
        assert!(s.ready().unwrap());
        let path = s.finalize(&d.path().join("out")).unwrap();
        assert_eq!(fs::read(path).unwrap(), input);
    }
}

#[test]
fn conflicting_symbol_is_not_overwritten() {
    let (d, _, frames) = setup(b"hello", "none");
    let mut s = Store::open(&d.path().join("state"), 1 << 30, None).unwrap();
    for f in frames.iter().filter(|f| f.object == 0) {
        s.ingest(f).unwrap();
    }
    let mut f = frames.iter().find(|f| f.object == 1).unwrap().clone();
    s.ingest(&f).unwrap();
    f.payload[0] ^= 1;
    assert_eq!(s.ingest(&f).unwrap(), Ingest::Conflict);
    for f in frames {
        s.ingest(&f).unwrap();
    }
    assert!(s.ready().unwrap());
}

#[test]
fn limits_binding_and_symlinks() {
    let (d, _, frames) = setup(&vec![1; 20000], "none");
    let state = d.path().join("state");
    let mut s = Store::open(&state, 1000, None).unwrap();
    let mut rejected = false;
    for f in frames.iter().filter(|f| f.object == 0) {
        if s.ingest(f).is_err() {
            rejected = true;
        }
    }
    assert!(rejected);
    assert!(s.active.is_none());
    drop(s);
    let mut s = Store::open(&state, 1 << 30, Some(transfer_id(b"other"))).unwrap();
    assert_eq!(s.ingest(&frames[0]).unwrap(), Ingest::Ignored);
    drop(s);
    assert!(Store::open(&state, 1 << 30, Some(frames[0].transfer)).is_err());
    #[cfg(unix)]
    {
        let state = d.path().join("linked");
        fs::create_dir(&state).unwrap();
        let victim = d.path().join("valuable");
        fs::write(&victim, b"keep").unwrap();
        std::os::unix::fs::symlink(&victim, state.join("receiver.lock")).unwrap();
        assert!(Store::open(&state, 1 << 30, None).is_err());
        assert_eq!(fs::read(victim).unwrap(), b"keep");
    }
}

#[test]
fn metadata_is_not_starved_by_a_full_late_join_cache() {
    let (d, _, frames) = setup(b"late join", "none");
    let mut s = Store::open(&d.path().join("state"), 1 << 30, None).unwrap();
    let f = frames.iter().find(|f| f.object == 1).unwrap();
    // Seed valid-size cached symbols efficiently to reproduce a late receiver
    // that missed metadata long enough to exhaust its 4MiB spool.
    let tx = s.conn.transaction().unwrap();
    for g in 0..196u32 {
        tx.execute(
            "INSERT INTO profiles VALUES(?1,1,?2,64,96,224)",
            rusqlite::params![f.transfer.as_slice(), g],
        )
        .unwrap();
        for symbol in 0..96u16 {
            tx.execute(
                "INSERT INTO shards VALUES(?1,1,?2,?3,?4,?5,0)",
                rusqlite::params![
                    f.transfer.as_slice(),
                    g,
                    symbol,
                    &f.payload,
                    crc32c::crc32c(&f.payload) as i64
                ],
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
    for f in frames.iter().filter(|f| f.object == 0) {
        s.ingest(f).unwrap();
    }
    assert!(
        s.active.is_some(),
        "a full data cache must not prevent receiving the manifest"
    );
    for f in frames {
        s.ingest(&f).unwrap();
    }
    assert!(s.ready().unwrap());
}

#[test]
fn one_mib_across_many_windows_with_erasures() {
    let input: Vec<_> = (0..1048576usize)
        .map(|i| ((i * 31 + i / 251) % 256) as u8)
        .collect();
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("large.bin");
    fs::write(&src, &input).unwrap();
    let p = d.path().join("large.tqtx");
    bundle::prepare(&src, &p, "none", 224).unwrap();
    let mut schedule = Schedule::new(Bundle::open(&p).unwrap(), None, Some(2), Some(79)).unwrap();
    let mut s = Store::open(&d.path().join("state"), 1 << 30, None).unwrap();
    let mut max_cache = 0;
    while let Some(f) = schedule.next_frame().unwrap() {
        if f.object == 1 && f.seq % 5 == 0 {
            continue;
        }
        s.ingest(&f).unwrap();
        if f.seq % 100 == 0 {
            max_cache = max_cache.max(s.status().unwrap().cached_symbols);
        }
        if s.ready().unwrap() {
            break;
        }
    }
    assert!(s.ready().unwrap());
    assert!(s.status().unwrap().total_generations > 64);
    assert!(max_cache <= 8 * 96);
    let out = s.finalize(&d.path().join("out")).unwrap();
    assert_eq!(fs::read(out).unwrap(), input);
}

#[test]
fn unknown_state_version_is_preserved() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("state.sqlite");
    {
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY,value BLOB);INSERT INTO settings VALUES('version',X'3939');CREATE TABLE valuable(x);INSERT INTO valuable VALUES('keep');").unwrap();
    }
    let before = fs::read(&db).unwrap();
    assert!(Store::open(d.path(), 1 << 30, None).is_err());
    assert_eq!(fs::read(&db).unwrap(), before);
}

#[test]
fn targeted_replay_only_emits_requested_generation() {
    let (_d, p, _) = setup(&vec![5; 30000], "none");
    let mut schedule =
        Schedule::new(Bundle::open(&p).unwrap(), Some("1"), Some(1), Some(91)).unwrap();
    let mut symbols = std::collections::BTreeSet::new();
    let mut transfer = None;
    while let Some(f) = schedule.next_frame().unwrap() {
        assert_eq!(*transfer.get_or_insert(f.transfer), f.transfer);
        if f.object == 1 {
            assert_eq!(f.generation, 1);
            symbols.insert(f.symbol);
        }
    }
    assert_eq!(symbols.len(), 96);
    assert!(Schedule::new(Bundle::open(&p).unwrap(), Some("3"), Some(1), None).is_err());
}
