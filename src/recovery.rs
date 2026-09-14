//! Private, bounded capture files and SQLite metadata. Capture bytes are never decoded.
use crate::{
    command::*,
    config::{Config, data_dir},
};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn private_dir(p: &Path) -> Result<()> {
    if fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("refusing symlink directory {}", p.display());
    }
    fs::create_dir_all(p)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn private_file(p: &Path) -> Result<File> {
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    Ok(o.open(p)?)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("missing parent")?;
    private_dir(parent)?;
    let tmp = parent.join(format!(".ttc-{}.tmp", id()?));
    let mut f = private_file(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(tmp);
        return Err(e.into());
    }
    Ok(())
}
pub fn id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("random ID: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn validate_id(s: &str) -> Result<()> {
    if s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("invalid capture ID: expected 32 hexadecimal characters");
    }
    Ok(())
}
pub struct Store {
    pub root: PathBuf,
    pub db: Connection,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub created: u64,
    pub cwd: String,
    pub command: String,
    pub outcome: ProcessOutcome,
    pub raw_bytes: u64,
    pub filtered_bytes: u64,
    pub duration_ms: u64,
    pub families: Vec<String>,
    pub partial: bool,
    #[serde(default)]
    pub execution_evidence: Option<ExecutionEvidence>,
}
impl Store {
    pub fn open() -> Result<Self> {
        Self::at(data_dir())
    }
    pub fn at(root: PathBuf) -> Result<Self> {
        private_dir(&root)?;
        private_dir(&root.join("captures"))?;
        let dbpath = root.join("history.sqlite3");
        if !dbpath.exists() {
            match private_file(&dbpath) {
                Ok(_) => {}
                Err(e) if dbpath.exists() => {
                    let _ = e;
                }
                Err(e) => return Err(e),
            }
        }
        if fs::symlink_metadata(&dbpath)?.file_type().is_symlink() {
            bail!("refusing symlink database");
        }
        let db = Connection::open(dbpath)?;
        db.busy_timeout(std::time::Duration::from_millis(500))?;
        db.execute_batch("PRAGMA journal_mode=DELETE; CREATE TABLE IF NOT EXISTS captures(id TEXT PRIMARY KEY, created INTEGER NOT NULL, bytes INTEGER NOT NULL, record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS metrics(id TEXT PRIMARY KEY, created INTEGER NOT NULL, raw INTEGER NOT NULL, filtered INTEGER NOT NULL, duration INTEGER NOT NULL, families TEXT NOT NULL); PRAGMA user_version=1;")?;
        Ok(Self { root, db })
    }
    fn quota_lock(&self) -> Result<File> {
        let p = self.root.join("quota.lock");
        if !p.exists() {
            let _ = private_file(&p);
        }
        if fs::symlink_metadata(&p)?.file_type().is_symlink() {
            bail!("refusing symlink lock");
        }
        let f = OpenOptions::new().read(true).write(true).open(p)?;
        f.lock_exclusive()?;
        Ok(f)
    }
    pub fn capture(&self, config: &Config) -> Result<Capture> {
        if config.recovery.max_entries == 0 {
            bail!("recovery entry capacity is zero");
        }
        let _lock = self.quota_lock()?;
        let raw_limit = config.recovery.max_capture_mb.saturating_mul(1024 * 1024);
        // Reserve worst-case space for incompressible data plus framing overhead.
        let reservation = raw_limit
            .saturating_add(raw_limit / 8)
            .saturating_add(1024 * 1024);
        let mut used = 0u64;
        for entry in fs::read_dir(self.root.join("captures"))? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().is_some_and(|s| s == "reserve") {
                let f = OpenOptions::new().read(true).write(true).open(&p)?;
                if f.try_lock_exclusive().is_ok() {
                    // The owning process has exited; only its uncommitted spool is removed.
                    let _ = fs::remove_file(p.with_extension("active"));
                    let _ = fs::remove_file(&p);
                } else {
                    used = used.saturating_add(fs::read_to_string(&p)?.parse::<u64>()?);
                }
            } else if p.extension().is_some_and(|s| s == "zst") {
                used = used.saturating_add(entry.metadata()?.len());
            }
        }
        if used.saturating_add(reservation)
            > config.recovery.max_total_mb.saturating_mul(1024 * 1024)
        {
            bail!("capture quota unavailable");
        }
        let id = id()?;
        let path = self.root.join("captures").join(format!("{id}.active"));
        let reservation_path = path.with_extension("reserve");
        let mut lease = private_file(&reservation_path)?;
        lease.lock_exclusive()?;
        write!(lease, "{reservation}")?;
        lease.flush()?;
        let encoder = zstd::stream::write::Encoder::new(private_file(&path)?, 1)?;
        Ok(Capture {
            id,
            path,
            encoder: Some(encoder),
            raw_bytes: 0,
            limit: raw_limit,
            binary: false,
            storage_limit: reservation,
            _lease: Lease {
                file: lease,
                path: reservation_path,
            },
        })
    }
    pub fn publish(&self, mut capture: Capture, record: &Record) -> Result<()> {
        validate_id(&record.id)?;
        if capture.id != record.id {
            bail!("capture/record ID mismatch");
        }
        let path = capture.path.clone();
        let _lock = self.quota_lock()?;
        capture.seal()?;
        let target = self
            .root
            .join("captures")
            .join(format!("{}.zst", record.id));
        fs::rename(path, &target)?;
        let bytes = fs::metadata(&target)?.len();
        if let Err(e) = self.db.execute(
            "INSERT INTO captures VALUES (?1,?2,?3,?4)",
            params![
                record.id,
                record.created,
                bytes,
                serde_json::to_string(record)?
            ],
        ) {
            return Err(e).context(format!("raw capture retained at {}", target.display()));
        }
        Ok(())
    }
    pub fn metric(&self, r: &Record) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO metrics VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                r.id,
                r.created,
                r.raw_bytes,
                r.filtered_bytes,
                r.duration_ms,
                serde_json::to_string(&r.families)?
            ],
        )?;
        self.db.execute("DELETE FROM metrics WHERE id IN (SELECT id FROM metrics ORDER BY created DESC LIMIT -1 OFFSET 100000)",[])?;
        Ok(())
    }
    pub fn record(&self, id: &str) -> Result<Record> {
        validate_id(id)?;
        let text: String = self
            .db
            .query_row("SELECT record FROM captures WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .context("capture missing or expired")?;
        Ok(serde_json::from_str(&text)?)
    }
    pub fn reader(&self, id: &str) -> Result<EventReader> {
        self.record(id)?;
        EventReader::open(&self.root.join("captures").join(format!("{id}.zst")))
    }
    pub fn clean(&self, c: &Config, purge: bool) -> Result<usize> {
        let _lock = self.quota_lock()?;
        if purge && let Ok(entries) = fs::read_dir(self.root.join("discovery")) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|s| s == "json")
                    && p.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                {
                    let _ = fs::remove_file(p);
                }
            }
        }
        let mut stmt = self
            .db
            .prepare("SELECT id,created,bytes FROM captures ORDER BY created DESC,rowid DESC")?;
        let rows: Vec<(String, u64, u64)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut total = 0u64;
        let mut count = 0;
        let mut removed = 0;
        for (id, created, bytes) in rows {
            total = total.saturating_add(bytes);
            count += 1;
            if purge
                || now().saturating_sub(created) > c.recovery.ttl_hours.saturating_mul(3600)
                || count > c.recovery.max_entries
                || total > c.recovery.max_total_mb.saturating_mul(1024 * 1024)
            {
                validate_id(&id)?;
                let _ = fs::remove_file(self.root.join("captures").join(format!("{id}.zst")));
                self.db.execute("DELETE FROM captures WHERE id=?1", [id])?;
                removed += 1;
            }
        }
        Ok(removed)
    }
    pub fn status(&self) -> Result<serde_json::Value> {
        let (count, bytes): (u64, u64) = self.db.query_row(
            "SELECT count(*),coalesce(sum(bytes),0) FROM captures",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(
            serde_json::json!({"schema_version":1,"entries":count,"compressed_bytes":bytes,"directory":self.root}),
        )
    }
    pub fn gain(&self, today: bool, history: bool) -> Result<serde_json::Value> {
        let since = if today { now() / 86400 * 86400 } else { 0 };
        let (n,raw,filtered):(u64,u64,u64)=self.db.query_row("SELECT count(*),coalesce(sum(raw),0),coalesce(sum(filtered),0) FROM metrics WHERE created>=?1",[since],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let mut v = serde_json::json!({"schema_version":1,"commands":n,"raw_bytes":raw,"filtered_bytes":filtered,"estimated_raw_tokens":raw.div_ceil(4),"estimated_filtered_tokens":filtered.div_ceil(4),"estimated_tokens_saved":raw.div_ceil(4) as i128-filtered.div_ceil(4) as i128,"reduction_percent":if raw==0{0.0}else{100.0*(1.0-filtered as f64/raw as f64)},"estimator":"bytes/4, not provider billing; today is UTC"});
        if history {
            let mut st = self.db.prepare(
                "SELECT created,raw,filtered,families FROM metrics ORDER BY created DESC LIMIT 100",
            )?;
            let rows=st.query_map([],|r|Ok(serde_json::json!({"timestamp":r.get::<_,u64>(0)?,"raw_bytes":r.get::<_,u64>(1)?,"filtered_bytes":r.get::<_,u64>(2)?,"families":r.get::<_,String>(3)?})))?.collect::<std::result::Result<Vec<_>,_>>()?;
            v["history"] = rows.into();
        }
        Ok(v)
    }
}
struct Lease {
    file: File,
    path: PathBuf,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = fs::remove_file(&self.path);
    }
}
pub struct Capture {
    _lease: Lease,
    pub id: String,
    pub path: PathBuf,
    encoder: Option<zstd::stream::write::Encoder<'static, File>>,
    pub raw_bytes: u64,
    limit: u64,
    storage_limit: u64,
    pub binary: bool,
}
impl Capture {
    pub fn write(&mut self, e: &OutputEvent) -> Result<()> {
        if e.bytes.len() > 65536 {
            bail!("event exceeds 64 KiB");
        }
        if self.encoder.as_ref().is_some_and(|w| {
            w.get_ref()
                .metadata()
                .is_ok_and(|m| m.len().saturating_add(131072) > self.storage_limit)
        }) {
            bail!("compressed capture quota reached");
        }
        if self.raw_bytes.saturating_add(e.bytes.len() as u64) > self.limit {
            bail!("capture size limit reached");
        }
        let w = self.encoder.as_mut().context("capture already finalized")?;
        w.write_all(&[if e.channel == Channel::Stdout { 0 } else { 1 }])?;
        w.write_all(&(e.bytes.len() as u32).to_le_bytes())?;
        w.write_all(&e.bytes)?;
        w.flush()?;
        self.raw_bytes += e.bytes.len() as u64;
        self.binary |= e.bytes.contains(&0);
        Ok(())
    }
    pub fn seal(&mut self) -> Result<()> {
        if let Some(w) = self.encoder.take() {
            w.finish()?.sync_all()?;
        }
        Ok(())
    }
    pub fn finish(mut self) -> Result<PathBuf> {
        self.seal()?;
        Ok(self.path.clone())
    }
}
pub struct EventReader {
    inner: zstd::stream::read::Decoder<'static, std::io::BufReader<File>>,
}
impl EventReader {
    pub fn open(path: &Path) -> Result<Self> {
        if fs::symlink_metadata(path)?.file_type().is_symlink() {
            bail!("refusing symlink capture");
        }
        Ok(Self {
            inner: zstd::stream::read::Decoder::new(File::open(path)?)?,
        })
    }
}
impl Iterator for EventReader {
    type Item = Result<OutputEvent>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut channel = [0];
        match self.inner.read(&mut channel) {
            Ok(0) => return None,
            Ok(_) => {}
            Err(e) => return Some(Err(e.into())),
        };
        Some((|| {
            if channel[0] > 1 {
                bail!("invalid channel");
            }
            let mut size = [0; 4];
            self.inner.read_exact(&mut size)?;
            let size = u32::from_le_bytes(size) as usize;
            if size > 65536 {
                bail!("invalid event length");
            }
            let mut bytes = vec![0; size];
            self.inner.read_exact(&mut bytes)?;
            Ok(OutputEvent {
                channel: if channel[0] == 0 {
                    Channel::Stdout
                } else {
                    Channel::Stderr
                },
                bytes,
            })
        })())
    }
}
