use crate::{
    bundle::{hash_reader, sync_dir, Manifest},
    fec,
    protocol::{self, Frame},
};
use anyhow::{ensure, Context, Result};
use fs2::FileExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const PRE_MANIFEST_BYTES: i64 = 4 * 1024 * 1024;

#[derive(Debug)]
pub struct FrameRejected(pub anyhow::Error);
impl std::fmt::Display for FrameRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for FrameRejected {}
fn protocol_check(condition: bool, message: &str) -> Result<()> {
    if !condition {
        return Err(FrameRejected(anyhow::anyhow!("{message}")).into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ingest {
    Added,
    Duplicate,
    Conflict,
    Ignored,
    Manifest,
    Group,
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub transfer_id: Option<String>,
    pub filename: Option<String>,
    pub completed_generations: u32,
    pub total_generations: u32,
    pub verified_transport_bytes: u64,
    pub cached_symbols: u64,
    pub missing_generations: String,
    pub finalized: bool,
    pub output: Option<String>,
}

pub struct Store {
    pub root: PathBuf,
    pub conn: Connection,
    pub active: Option<([u8; 16], Manifest)>,
    max_bytes: u64,
    _lock: File,
}

fn reject_symlink(path: &Path) -> Result<()> {
    if let Ok(m) = fs::symlink_metadata(path) {
        ensure!(
            m.is_file() && !m.file_type().is_symlink(),
            "state path is not a regular file: {}",
            path.display()
        );
    }
    Ok(())
}

impl Store {
    pub fn open(root: &Path, max_bytes: u64, expected: Option<[u8; 16]>) -> Result<Self> {
        ensure!(max_bytes > 0, "max bytes must be positive");
        let new_root = !root.exists();
        fs::create_dir_all(root)?;
        #[cfg(unix)]
        if new_root {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        }
        let lockpath = root.join("receiver.lock");
        reject_symlink(&lockpath)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lockpath)?;
        lock.try_lock_exclusive()
            .context("another receiver is using this state directory")?;
        let db = root.join("state.sqlite");
        reject_symlink(&db)?;
        reject_symlink(&root.join("state.sqlite-wal"))?;
        reject_symlink(&root.join("state.sqlite-shm"))?;
        let existing = fs::metadata(&db).is_ok_and(|m| m.len() > 0);
        let conn = Connection::open(db)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        if existing {
            let v: Option<Vec<u8>> = conn
                .query_row("SELECT value FROM settings WHERE key='version'", [], |r| {
                    r.get(0)
                })
                .optional()
                .context("not a recognized tqr state database")?;
            ensure!(
                v.as_deref() == Some(b"1"),
                "unsupported or missing receiver state version"
            );
        }
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
            BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value BLOB NOT NULL);
            INSERT OR IGNORE INTO settings VALUES('version',X'31');
            CREATE TABLE IF NOT EXISTS manifests(id BLOB PRIMARY KEY,raw BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS profiles(id BLOB,obj INTEGER,g INTEGER,k INTEGER,n INTEGER,size INTEGER,PRIMARY KEY(id,obj,g));
            CREATE TABLE IF NOT EXISTS shards(id BLOB,obj INTEGER,g INTEGER,s INTEGER,payload BLOB NOT NULL,crc INTEGER NOT NULL,conflict INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(id,obj,g,s));
            CREATE TABLE IF NOT EXISTS completed(id BLOB,g INTEGER,len INTEGER,sha TEXT,PRIMARY KEY(id,g));
            CREATE TABLE IF NOT EXISTS exports(id BLOB PRIMARY KEY,path TEXT NOT NULL);
            COMMIT;")?;
        let version: Option<Vec<u8>> = conn
            .query_row("SELECT value FROM settings WHERE key='version'", [], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(v) = version {
            ensure!(v == b"1", "unsupported receiver state version");
        } else {
            conn.execute(
                "INSERT INTO settings VALUES('version',?1)",
                [b"1".as_slice()],
            )?;
        }
        let integrity: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        ensure!(integrity == "ok", "SQLite quick_check failed: {integrity}");
        let selected: Option<Vec<u8>> = conn
            .query_row("SELECT value FROM settings WHERE key='selected'", [], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(id) = &selected {
            ensure!(id.len() == 16, "invalid persisted transfer ID");
        }
        if let (Some(want), Some(have)) = (expected, selected.as_ref()) {
            ensure!(
                want.as_slice() == have,
                "state is bound to another transfer"
            );
        }
        if selected.is_none() {
            if let Some(want) = expected {
                conn.execute(
                    "INSERT INTO settings VALUES('selected',?1)",
                    [want.as_slice()],
                )?;
            }
        }
        let raw:Option<Vec<u8>>=conn.query_row("SELECT m.raw FROM manifests m JOIN settings s ON s.key='selected' AND s.value=m.id",[],|r|r.get(0)).optional()?;
        if let Some(raw) = &raw {
            let saved: Option<Vec<u8>> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='manifest_sha256'",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            ensure!(
                saved.as_deref() == Some(protocol::digest(raw).as_slice()),
                "persisted full manifest SHA-256 mismatch"
            );
        }
        let active = raw
            .map(|r| Ok::<_, anyhow::Error>((protocol::transfer_id(&r), Manifest::parse(&r)?)))
            .transpose()?;
        if let (Some((id, _)), Some(selected)) = (&active, &selected) {
            ensure!(id.as_slice() == selected, "persisted manifest ID mismatch");
        }
        let mut s = Self {
            root: root.to_owned(),
            conn,
            active,
            max_bytes,
            _lock: lock,
        };
        if let Some((_, m)) = &s.active {
            s.check_limit(m)?;
            s.revalidate_completed()?;
            s.recover_cached()?;
        }
        Ok(s)
    }
    fn check_limit(&self, m: &Manifest) -> Result<()> {
        ensure!(
            m.original_bytes <= self.max_bytes && m.transport_bytes <= self.max_bytes,
            "manifest exceeds --max-bytes ({})",
            self.max_bytes
        );
        Ok(())
    }
    pub fn part_path(&self, id: &[u8; 16]) -> PathBuf {
        self.root.join(format!("{}.part", hex::encode(id)))
    }
    fn part(&self, id: &[u8; 16]) -> Result<File> {
        let p = self.part_path(id);
        reject_symlink(&p)?;
        Ok(OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(p)?)
    }
    fn selected(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key='selected'", [], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn ingest(&mut self, f: &Frame) -> Result<Ingest> {
        f.validate().map_err(FrameRejected)?;
        if self.selected()?.is_some_and(|id| id != f.transfer) {
            return Ok(Ingest::Ignored);
        }
        if let Some((id, m)) = &self.active {
            ensure!(id == &f.transfer, "internal transfer mismatch");
            if f.object == 1 {
                protocol_check(
                    f.symbol_bytes == m.symbol_bytes && f.generation < m.generations(),
                    "frame disagrees with manifest",
                )?;
            }
            if f.object == 0 {
                return Ok(Ingest::Duplicate);
            }
            let done: bool = self.conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM completed WHERE id=?1 AND g=?2)",
                params![f.transfer.as_slice(), f.generation],
                |r| r.get(0),
            )?;
            if done {
                return Ok(Ingest::Duplicate);
            }
        }
        let existing: Option<(u8, u8, u16)> = self
            .conn
            .query_row(
                "SELECT k,n,size FROM profiles WHERE id=?1 AND obj=?2 AND g=?3",
                params![f.transfer.as_slice(), f.object, f.generation],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some(profile) = existing {
            protocol_check(
                profile == (f.k, f.n, f.symbol_bytes),
                "inconsistent generation parameters",
            )?;
        }
        let old: Option<(Vec<u8>, i64, bool)> = self
            .conn
            .query_row(
                "SELECT payload,crc,conflict FROM shards WHERE id=?1 AND obj=?2 AND g=?3 AND s=?4",
                params![f.transfer.as_slice(), f.object, f.generation, f.symbol],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((old, crc, conflict)) = old {
            if crc32c::crc32c(&old) as i64 != crc {
                self.conn.execute(
                    "DELETE FROM shards WHERE id=?1 AND obj=?2 AND g=?3 AND s=?4",
                    params![f.transfer.as_slice(), f.object, f.generation, f.symbol],
                )?;
            } else if conflict || old != f.payload {
                self.conn.execute(
                    "UPDATE shards SET conflict=1 WHERE id=?1 AND obj=?2 AND g=?3 AND s=?4",
                    params![f.transfer.as_slice(), f.object, f.generation, f.symbol],
                )?;
                return Ok(Ingest::Conflict);
            } else {
                return Ok(Ingest::Duplicate);
            }
        }
        if self.active.is_none() {
            let mut bytes: i64 = self.conn.query_row(
                "SELECT COALESCE(SUM(length(payload)),0) FROM shards",
                [],
                |r| r.get(0),
            )?;
            let mut profiles: i64 =
                self.conn
                    .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))?;
            // Metadata must remain receivable after the bounded data cache fills.
            if f.object == 0
                && (bytes + f.payload.len() as i64 > PRE_MANIFEST_BYTES
                    || (existing.is_none() && profiles >= 256))
            {
                let tx = self.conn.transaction()?;
                tx.execute("DELETE FROM shards WHERE obj=1", [])?;
                tx.execute("DELETE FROM profiles WHERE obj=1", [])?;
                tx.commit()?;
                bytes = self.conn.query_row(
                    "SELECT COALESCE(SUM(length(payload)),0) FROM shards",
                    [],
                    |r| r.get(0),
                )?;
                profiles = self
                    .conn
                    .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))?;
                if bytes + f.payload.len() as i64 > PRE_MANIFEST_BYTES
                    || (existing.is_none() && profiles >= 256)
                {
                    let tx = self.conn.transaction()?;
                    tx.execute("DELETE FROM shards WHERE id!=?1", [f.transfer.as_slice()])?;
                    tx.execute("DELETE FROM profiles WHERE id!=?1", [f.transfer.as_slice()])?;
                    tx.commit()?;
                    bytes = self.conn.query_row(
                        "SELECT COALESCE(SUM(length(payload)),0) FROM shards",
                        [],
                        |r| r.get(0),
                    )?;
                    profiles = self
                        .conn
                        .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))?;
                }
            }
            if bytes + f.payload.len() as i64 > PRE_MANIFEST_BYTES {
                return Ok(Ingest::Ignored);
            }
            if existing.is_none() && profiles >= 256 {
                return Ok(Ingest::Ignored);
            }
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO profiles VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                f.transfer.as_slice(),
                f.object,
                f.generation,
                f.k,
                f.n,
                f.symbol_bytes
            ],
        )?;
        tx.execute(
            "INSERT INTO shards(id,obj,g,s,payload,crc) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                f.transfer.as_slice(),
                f.object,
                f.generation,
                f.symbol,
                &f.payload,
                crc32c::crc32c(&f.payload) as i64
            ],
        )?;
        tx.commit()?;
        if f.object == 1 && self.active.is_none() {
            return Ok(Ingest::Added);
        }
        self.try_group(&f.transfer, f.object, f.generation)
    }

    fn try_group(&mut self, id: &[u8; 16], obj: u32, g: u32) -> Result<Ingest> {
        let profile: Option<(u8, u8, u16)> = self
            .conn
            .query_row(
                "SELECT k,n,size FROM profiles WHERE id=?1 AND obj=?2 AND g=?3",
                params![id.as_slice(), obj, g],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((k, n, size)) = profile else {
            return Ok(Ingest::Added);
        };
        ensure!(
            k > 0
                && k <= 64
                && k < n
                && n <= 96
                && (64..=protocol::MAX_SYMBOL).contains(&(size as usize)),
            "corrupt persisted group profile"
        );
        let mut shards = vec![None; n as usize];
        let stored = {
            let mut st = self.conn.prepare(
                "SELECT s,payload,crc FROM shards WHERE id=?1 AND obj=?2 AND g=?3 AND conflict=0",
            )?;
            let rows = st.query_map(params![id.as_slice(), obj, g], |r| {
                Ok((
                    r.get::<_, usize>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (s, p, crc) in stored {
            if s < shards.len() && p.len() == size as usize && crc32c::crc32c(&p) as i64 == crc {
                shards[s] = Some(p);
            } else {
                self.conn.execute(
                    "DELETE FROM shards WHERE id=?1 AND obj=?2 AND g=?3 AND s=?4",
                    params![id.as_slice(), obj, g, s as u32],
                )?;
            }
        }
        if shards.iter().flatten().count() < k as usize {
            return Ok(Ingest::Added);
        }
        let data = match fec::decode(shards, k, n, size as usize) {
            Ok(d) => d,
            Err(e) => {
                self.conn.execute(
                    "DELETE FROM shards WHERE id=?1 AND obj=?2 AND g=?3",
                    params![id.as_slice(), obj, g],
                )?;
                eprintln!("generation {obj}:{g} rejected ({e}); waiting for replay");
                return Ok(Ingest::Conflict);
            }
        };
        if obj == 0 {
            let checked = (|| -> Result<Manifest> {
                ensure!(
                    data.len() <= protocol::MAX_MANIFEST && protocol::transfer_id(&data) == *id,
                    "manifest transfer ID mismatch"
                );
                let m = Manifest::parse(&data)?;
                self.check_limit(&m)?;
                ensure!(
                    m.symbol_bytes == size,
                    "manifest/frame symbol size mismatch"
                );
                Ok(m)
            })();
            let m = match checked {
                Ok(m) => m,
                Err(e) => {
                    let tx = self.conn.transaction()?;
                    tx.execute("DELETE FROM shards WHERE id=?1 AND obj=0", [id.as_slice()])?;
                    tx.execute(
                        "DELETE FROM profiles WHERE id=?1 AND obj=0",
                        [id.as_slice()],
                    )?;
                    tx.commit()?;
                    return Err(FrameRejected(e).into());
                }
            };
            let mut part = self.part(id)?;
            part.set_len(m.transport_bytes)?;
            part.flush()?;
            part.sync_all()?;
            sync_dir(&self.root)?;
            let tx = self.conn.transaction()?;
            tx.execute(
                "INSERT OR REPLACE INTO settings VALUES('selected',?1)",
                [id.as_slice()],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO manifests VALUES(?1,?2)",
                params![id.as_slice(), &data],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO settings VALUES('manifest_sha256',?1)",
                [protocol::digest(&data).as_slice()],
            )?;
            tx.execute(
                "DELETE FROM shards WHERE id!=?1 OR obj=0 OR g>=?2",
                params![id.as_slice(), m.generations()],
            )?;
            tx.execute(
                "DELETE FROM profiles WHERE id!=?1 OR obj=0 OR g>=?2 OR size!=?3",
                params![id.as_slice(), m.generations(), m.symbol_bytes],
            )?;
            tx.execute("DELETE FROM shards WHERE NOT EXISTS(SELECT 1 FROM profiles p WHERE p.id=shards.id AND p.obj=shards.obj AND p.g=shards.g)",[])?;
            tx.commit()?;
            self.active = Some((*id, m));
            self.recover_cached()?;
            return Ok(Ingest::Manifest);
        }
        let (active, m) = self
            .active
            .as_ref()
            .context("data recovery requires manifest")?;
        ensure!(
            active == id && m.symbol_bytes == size && m.k == k && m.n == n,
            "persisted group disagrees with manifest"
        );
        if data.len() != m.generation_len(g)? {
            self.conn.execute(
                "DELETE FROM shards WHERE id=?1 AND obj=1 AND g=?2",
                params![id.as_slice(), g],
            )?;
            return Err(
                FrameRejected(anyhow::anyhow!("recovered generation length mismatch")).into(),
            );
        }
        let mut part = self.part(id)?;
        part.seek(SeekFrom::Start(u64::from(g) * m.group_capacity() as u64))?;
        part.write_all(&data)?;
        part.sync_all()?;
        // Data reaches stable storage before SQLite advertises completion.
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO completed VALUES(?1,?2,?3,?4)",
            params![
                id.as_slice(),
                g,
                data.len() as u64,
                hex::encode(protocol::digest(&data))
            ],
        )?;
        tx.execute(
            "DELETE FROM shards WHERE id=?1 AND obj=1 AND g=?2",
            params![id.as_slice(), g],
        )?;
        tx.execute(
            "DELETE FROM profiles WHERE id=?1 AND obj=1 AND g=?2",
            params![id.as_slice(), g],
        )?;
        tx.commit()?;
        Ok(Ingest::Group)
    }

    fn recover_cached(&mut self) -> Result<()> {
        let Some((id, _)) = self.active.as_ref() else {
            return Ok(());
        };
        let id = *id;
        let gs = {
            let mut st = self
                .conn
                .prepare("SELECT g FROM profiles WHERE id=?1 AND obj=1 ORDER BY g")?;
            let rows = st.query_map([id.as_slice()], |r| r.get::<_, u32>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for g in gs {
            self.try_group(&id, 1, g)?;
        }
        Ok(())
    }
    fn revalidate_completed(&mut self) -> Result<()> {
        let Some((id, m)) = self.active.as_ref() else {
            return Ok(());
        };
        let id = *id;
        let m = m.clone();
        let rows = {
            let mut st = self
                .conn
                .prepare("SELECT g,len,sha FROM completed WHERE id=?1")?;
            let rows = st.query_map([id.as_slice()], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, u64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut part = self.part(&id)?;
        let mut invalid = false;
        for (g, len, sha) in rows {
            let good = if let Ok(want) = m.generation_len(g) {
                if len == want as u64 {
                    part.seek(SeekFrom::Start(u64::from(g) * m.group_capacity() as u64))?;
                    let (actual, h) = hash_reader((&mut part).take(len))?;
                    actual == len && h == sha
                } else {
                    false
                }
            } else {
                false
            };
            if !good {
                invalid = true;
                self.conn.execute(
                    "DELETE FROM completed WHERE id=?1 AND g=?2",
                    params![id.as_slice(), g],
                )?;
            }
        }
        if invalid {
            self.conn
                .execute("DELETE FROM exports WHERE id=?1", [id.as_slice()])?;
        }
        part.set_len(m.transport_bytes)?;
        Ok(())
    }
    pub fn ready(&self) -> Result<bool> {
        let Some((id, m)) = &self.active else {
            return Ok(false);
        };
        let count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM completed WHERE id=?1",
            [id.as_slice()],
            |r| r.get(0),
        )?;
        Ok(count == m.generations())
    }

    pub fn finalize(&mut self, output: &Path) -> Result<PathBuf> {
        ensure!(self.ready()?, "transfer is incomplete");
        let (id, m) = self.active.as_ref().unwrap();
        let id = *id;
        let m = m.clone();
        let mut part = self.part(&id)?;
        part.seek(SeekFrom::Start(0))?;
        let (len, hash) = hash_reader(&mut part)?;
        ensure!(
            len == m.transport_bytes && hash == m.transport_sha256,
            "final transport SHA-256 mismatch"
        );
        fs::create_dir_all(output)?;
        let target = output.join(&m.filename);
        if target.exists() || fs::symlink_metadata(&target).is_ok() {
            reject_symlink(&target)?;
            let (len, hash) = hash_reader(File::open(&target)?)?;
            ensure!(
                len == m.original_bytes && hash == m.original_sha256,
                "output already exists with different contents: {}",
                target.display()
            );
        } else {
            let mut temp = NamedTempFile::new_in(output)?;
            part.seek(SeekFrom::Start(0))?;
            let mut reader: Box<dyn Read> = if m.compression == "zstd" {
                Box::new(zstd::stream::read::Decoder::new(part)?)
            } else {
                Box::new(part)
            };
            let count = std::io::copy(
                &mut reader.by_ref().take(
                    m.original_bytes
                        .checked_add(1)
                        .context("original size overflow")?,
                ),
                &mut temp,
            )?;
            ensure!(count == m.original_bytes, "decompressed length mismatch");
            temp.as_file_mut().seek(SeekFrom::Start(0))?;
            let (len, hash) = hash_reader(temp.as_file_mut())?;
            ensure!(
                len == m.original_bytes && hash == m.original_sha256,
                "original file SHA-256 mismatch"
            );
            temp.as_file().sync_all()?;
            temp.persist_noclobber(&target)
                .map_err(|e| e.error)
                .context("publish without overwriting output")?;
            sync_dir(output)?;
        }
        let absolute_target = fs::canonicalize(&target)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO exports VALUES(?1,?2)",
            params![id.as_slice(), absolute_target.to_string_lossy()],
        )?;
        Ok(target)
    }

    pub fn status(&self) -> Result<Status> {
        read_status(&self.conn)
    }
}

pub fn status(root: &Path) -> Result<Status> {
    let conn = Connection::open_with_flags(
        root.join("state.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    read_status(&conn)
}

fn read_status(conn: &Connection) -> Result<Status> {
    let cached_symbols: u64 = conn.query_row("SELECT COUNT(*) FROM shards", [], |r| r.get(0))?;
    let raw:Option<(Vec<u8>,Vec<u8>)>=conn.query_row("SELECT m.id,m.raw FROM manifests m JOIN settings s ON s.key='selected' AND s.value=m.id",[],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let mut s = Status {
        transfer_id: None,
        filename: None,
        completed_generations: 0,
        total_generations: 0,
        verified_transport_bytes: 0,
        cached_symbols,
        missing_generations: String::new(),
        finalized: false,
        output: None,
    };
    if let Some((id, raw)) = raw {
        let m = Manifest::parse(&raw)?;
        let saved: Option<Vec<u8>> = conn
            .query_row(
                "SELECT value FROM settings WHERE key='manifest_sha256'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(
            saved.as_deref() == Some(protocol::digest(&raw).as_slice()),
            "persisted full manifest SHA-256 mismatch"
        );
        ensure!(
            protocol::transfer_id(&raw).as_slice() == id,
            "persisted manifest ID mismatch"
        );
        let mut st = conn.prepare("SELECT g,len FROM completed WHERE id=?1 ORDER BY g")?;
        let done: Vec<(u32, u64)> = st
            .query_map([&id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let set: std::collections::BTreeSet<_> = done.iter().map(|(g, _)| *g).collect();
        let mut ranges = vec![];
        let mut start = None;
        for g in 0..m.generations() {
            if !set.contains(&g) {
                if start.is_none() {
                    start = Some(g);
                }
            } else if let Some(a) = start.take() {
                ranges.push(range(a, g - 1));
            }
        }
        if let Some(a) = start {
            ranges.push(range(a, m.generations() - 1));
        }
        s.transfer_id = Some(hex::encode(id.clone()));
        s.total_generations = m.generations();
        s.filename = Some(m.filename.clone());
        s.completed_generations = done.len() as u32;
        s.verified_transport_bytes = done.iter().map(|(_, n)| n).sum();
        s.missing_generations = ranges.join(",");
        s.output = conn
            .query_row("SELECT path FROM exports WHERE id=?1", [&id], |r| r.get(0))
            .optional()?;
        if let Some(path) = &s.output {
            let regular = fs::symlink_metadata(path).is_ok_and(|meta| {
                meta.is_file() && !meta.file_type().is_symlink() && meta.len() == m.original_bytes
            });
            if regular {
                s.finalized = File::open(path)
                    .ok()
                    .and_then(|f| hash_reader(f).ok())
                    .is_some_and(|(len, h)| len == m.original_bytes && h == m.original_sha256);
            }
        }
    }
    Ok(s)
}
fn range(a: u32, b: u32) -> String {
    if a == b {
        a.to_string()
    } else {
        format!("{a}-{b}")
    }
}
