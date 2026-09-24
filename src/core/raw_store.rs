//! Bounded, tagged capture of the original output bytes.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::config::Config;

const HEADER_SLOT: usize = 96;
const HEADER: usize = HEADER_SLOT * 2;
const RECORD_HEADER: usize = 5;
const CHUNK: usize = 32 * 1024;
const JOURNAL: usize = CHUNK + RECORD_HEADER;
const MAGIC: &[u8; 8] = b"TTCRAW2\0";

#[derive(Clone, Copy)]
struct RingState {
    start: usize,
    end: usize,
    used: usize,
    dropped: u64,
}

#[derive(Clone, Copy)]
struct DiskHeader {
    generation: u64,
    capacity: usize,
    state: RingState,
    journal_at: usize,
    journal_len: usize,
    active: bool,
}

impl DiskHeader {
    fn encode(self) -> [u8; HEADER_SLOT] {
        let mut bytes = [0; HEADER_SLOT];
        bytes[..8].copy_from_slice(MAGIC);
        for (index, value) in [
            self.generation,
            self.capacity as u64,
            self.state.start as u64,
            self.state.end as u64,
            self.state.used as u64,
            self.state.dropped,
            self.journal_at as u64,
            self.journal_len as u64,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[72] = u8::from(self.active);
        let digest = checksum(&bytes[..80]);
        bytes[80..88].copy_from_slice(&digest.to_le_bytes());
        bytes
    }

    fn decode(bytes: &[u8; HEADER_SLOT]) -> Option<Self> {
        if &bytes[..8] != MAGIC
            || u64::from_le_bytes(bytes[80..88].try_into().ok()?) != checksum(&bytes[..80])
            || bytes[72] > 1
        {
            return None;
        }
        let number = |index: usize| {
            u64::from_le_bytes(bytes[8 + index * 8..16 + index * 8].try_into().unwrap())
        };
        Some(Self {
            generation: number(0),
            capacity: usize::try_from(number(1)).ok()?,
            state: RingState {
                start: usize::try_from(number(2)).ok()?,
                end: usize::try_from(number(3)).ok()?,
                used: usize::try_from(number(4)).ok()?,
                dropped: number(5),
            },
            journal_at: usize::try_from(number(6)).ok()?,
            journal_len: usize::try_from(number(7)).ok()?,
            active: bytes[72] == 1,
        })
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stream {
    Stdout = 1,
    Stderr = 2,
}

impl Stream {
    fn from_byte(byte: u8) -> io::Result<Self> {
        match byte {
            1 => Ok(Self::Stdout),
            2 => Ok(Self::Stderr),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid stream tag",
            )),
        }
    }
}

enum Backend {
    Memory(Vec<u8>),
    File(File),
}

pub(crate) struct Capture {
    roots: StorePaths,
    capacity: usize,
    start: usize,
    end: usize,
    used: usize,
    dropped: u64,
    generation: u64,
    backend: Backend,
    pending_path: Option<PathBuf>,
    final_path: Option<PathBuf>,
    id: Option<String>,
    #[cfg(test)]
    fail_append_after_data: bool,
    #[cfg(test)]
    fail_finish_rename: bool,
}

impl Capture {
    pub(crate) fn new(config: Config, roots: StorePaths) -> Self {
        let capacity = config.max_bytes() - HEADER - JOURNAL;
        Self {
            roots,
            capacity,
            start: 0,
            end: 0,
            used: 0,
            dropped: 0,
            generation: 0,
            backend: Backend::Memory(vec![0; capacity]),
            pending_path: None,
            final_path: None,
            id: None,
            #[cfg(test)]
            fail_append_after_data: false,
            #[cfg(test)]
            fail_finish_rename: false,
        }
    }

    pub(crate) fn record(&mut self, stream: Stream, bytes: &[u8]) -> io::Result<()> {
        for chunk in bytes.chunks(CHUNK) {
            if chunk.is_empty() {
                continue;
            }
            let previous = self.state();
            let needed = RECORD_HEADER + chunk.len();
            while self.used + needed > self.capacity {
                let mut header = [0; RECORD_HEADER];
                self.read_at(self.start, &mut header)?;
                Stream::from_byte(header[0])?;
                let length = u32::from_le_bytes(header[1..5].try_into().unwrap()) as usize;
                if length == 0 || length > CHUNK || RECORD_HEADER + length > self.used {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid capture record",
                    ));
                }
                self.start = (self.start + RECORD_HEADER + length) % self.capacity;
                self.used -= RECORD_HEADER + length;
                self.dropped += length as u64;
            }
            let mut encoded = Vec::with_capacity(needed);
            encoded.push(stream as u8);
            encoded.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            encoded.extend_from_slice(chunk);
            if matches!(self.backend, Backend::File(_)) {
                if let Err(error) = self.append_file(&encoded, previous) {
                    self.restore(previous);
                    return Err(error);
                }
            } else {
                self.write_at(self.end, &encoded)?;
                self.end = (self.end + needed) % self.capacity;
                self.used += needed;
            }
        }
        Ok(())
    }

    fn state(&self) -> RingState {
        RingState {
            start: self.start,
            end: self.end,
            used: self.used,
            dropped: self.dropped,
        }
    }

    fn restore(&mut self, state: RingState) {
        self.start = state.start;
        self.end = state.end;
        self.used = state.used;
        self.dropped = state.dropped;
    }

    fn append_file(&mut self, encoded: &[u8], previous: RingState) -> io::Result<()> {
        let mut backup = vec![0; encoded.len()];
        self.read_at(previous.end, &mut backup)?;
        let Backend::File(file) = &mut self.backend else {
            unreachable!()
        };
        // Write the undo bytes before publishing an intent header. Replay uses
        // that header and the undo bytes if the ring write or commit fails.
        file.seek(SeekFrom::Start((HEADER + self.capacity) as u64))?;
        file.write_all(&backup)?;
        file.flush()?;

        let intent = DiskHeader {
            generation: self.generation + 1,
            capacity: self.capacity,
            state: previous,
            journal_at: previous.end,
            journal_len: encoded.len(),
            active: true,
        };
        self.write_disk_header(1, intent)?;
        self.write_at(previous.end, encoded)?;
        #[cfg(test)]
        if self.fail_append_after_data {
            return Err(io::Error::other("injected capture append error"));
        }
        self.end = (previous.end + encoded.len()) % self.capacity;
        self.used += encoded.len();
        let committed = DiskHeader {
            generation: self.generation + 2,
            capacity: self.capacity,
            state: self.state(),
            journal_at: 0,
            journal_len: 0,
            active: false,
        };
        self.write_disk_header(0, committed)?;
        self.generation += 2;
        Ok(())
    }

    pub(crate) fn enable(&mut self, config: Config) -> io::Result<&str> {
        if self.id.is_some() {
            return Ok(self.id.as_deref().unwrap());
        }
        let mut last_error = None;
        for root in self.roots.0.clone() {
            match self.enable_at(&root, config) {
                Ok(()) => return Ok(self.id.as_deref().unwrap()),
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| io::Error::other("no raw storage available")))
    }

    fn enable_at(&mut self, root: &Path, config: Config) -> io::Result<()> {
        prepare_root(root)?;
        cleanup_at(root, config);
        let mut random = [0u8; 16];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        let id = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let pending_path = root.join(format!("{id}.part"));
        let final_path = root.join(format!("{id}.raw"));
        let mut file = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending_path)?;
        let result = (|| {
            file.set_len(config.max_bytes() as u64)?;
            if let Backend::Memory(data) = &self.backend {
                let first = self.used.min(self.capacity - self.start);
                if first > 0 {
                    file.seek(SeekFrom::Start((HEADER + self.start) as u64))?;
                    file.write_all(&data[self.start..self.start + first])?;
                }
                if self.used > first {
                    file.seek(SeekFrom::Start(HEADER as u64))?;
                    file.write_all(&data[..self.used - first])?;
                }
            }
            let header = DiskHeader {
                generation: 0,
                capacity: self.capacity,
                state: self.state(),
                journal_at: 0,
                journal_len: 0,
                active: false,
            }
            .encode();
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&header)?;
            file.flush()
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&pending_path);
            return Err(error);
        }
        self.backend = Backend::File(file);
        self.pending_path = Some(pending_path);
        self.final_path = Some(final_path);
        self.id = Some(id);
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> io::Result<Option<&str>> {
        let Some(pending) = self.pending_path.as_ref() else {
            return Ok(None);
        };
        let pending = pending.clone();
        let flush_result = match &mut self.backend {
            Backend::File(file) => file.flush(),
            Backend::Memory(_) => unreachable!(),
        };
        if let Err(error) = flush_result {
            if self.pending_is_replayable(&pending) {
                return Ok(self.id.as_deref());
            }
            return Err(error);
        }
        #[cfg(test)]
        let rename_result = if self.fail_finish_rename {
            Err(io::Error::other("injected capture rename error"))
        } else {
            fs::rename(&pending, self.final_path.as_ref().unwrap())
        };
        #[cfg(not(test))]
        let rename_result = fs::rename(&pending, self.final_path.as_ref().unwrap());
        match rename_result {
            Ok(()) => self.pending_path = None,
            Err(error) if !self.pending_is_replayable(&pending) => return Err(error),
            Err(_) => {}
        }
        Ok(self.id.as_deref())
    }

    fn pending_is_replayable(&self, pending: &Path) -> bool {
        File::open(pending).is_ok_and(|file| {
            replay_file(
                file,
                pending,
                Selection::Stdout,
                Some(0),
                &mut io::sink(),
                &mut io::sink(),
            )
            .is_ok()
        })
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn inject_append_failure(&mut self) {
        self.fail_append_after_data = true;
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn inject_rename_failure(&mut self) {
        self.fail_finish_rename = true;
    }

    fn write_disk_header(&mut self, slot: usize, header: DiskHeader) -> io::Result<()> {
        if let Backend::File(file) = &mut self.backend {
            file.seek(SeekFrom::Start((slot * HEADER_SLOT) as u64))?;
            file.write_all(&header.encode())?;
            file.flush()?;
        }
        Ok(())
    }

    fn write_at(&mut self, at: usize, bytes: &[u8]) -> io::Result<()> {
        let first = bytes.len().min(self.capacity - at);
        match &mut self.backend {
            Backend::Memory(data) => {
                data[at..at + first].copy_from_slice(&bytes[..first]);
                data[..bytes.len() - first].copy_from_slice(&bytes[first..]);
            }
            Backend::File(file) => {
                file.seek(SeekFrom::Start((HEADER + at) as u64))?;
                file.write_all(&bytes[..first])?;
                if first < bytes.len() {
                    file.seek(SeekFrom::Start(HEADER as u64))?;
                    file.write_all(&bytes[first..])?;
                }
            }
        }
        Ok(())
    }

    fn read_at(&mut self, at: usize, bytes: &mut [u8]) -> io::Result<()> {
        let length = bytes.len();
        let first = bytes.len().min(self.capacity - at);
        match &mut self.backend {
            Backend::Memory(data) => {
                bytes[..first].copy_from_slice(&data[at..at + first]);
                bytes[first..].copy_from_slice(&data[..length - first]);
            }
            Backend::File(file) => {
                file.seek(SeekFrom::Start((HEADER + at) as u64))?;
                file.read_exact(&mut bytes[..first])?;
                if first < bytes.len() {
                    file.seek(SeekFrom::Start(HEADER as u64))?;
                    file.read_exact(&mut bytes[first..])?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct StorePaths(pub [PathBuf; 2]);

impl StorePaths {
    pub(crate) fn from_env() -> io::Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/state"));
        let uid = nix::unistd::getuid().as_raw();
        // Test harnesses point this at a task-scoped directory. Normal runs
        // use /tmp regardless of an invocation's TMPDIR value.
        let temporary = std::env::var_os("TTC_INTERNAL_TEST_TMP_ROOT")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        Ok(Self([
            state.join("ttc/runs"),
            temporary.join(format!("ttc-{uid}/runs")),
        ]))
    }
}

fn prepare_root(root: &Path) -> io::Result<()> {
    let parent = root.parent().unwrap();
    fs::create_dir_all(parent.parent().unwrap())?;
    for directory in [parent, root] {
        match DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir()
            || metadata.uid() != nix::unistd::getuid().as_raw()
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe raw directory",
            ));
        }
    }
    Ok(())
}

fn cleanup_at(root: &Path, config: Config) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let now = SystemTime::now();
    let limit = Duration::from_secs(config.retention_hours * 3600);
    for entry in entries.flatten() {
        let path = entry.path();
        if !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("raw" | "part")
        ) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !valid_id(stem) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.uid() != nix::unistd::getuid().as_raw() {
            continue;
        }
        if metadata
            .modified()
            .ok()
            .and_then(|time| now.duration_since(time).ok())
            .is_some_and(|age| age > limit)
        {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn cleanup(config: Config) {
    if let Ok(paths) = StorePaths::from_env() {
        cleanup_paths(config, &paths);
    }
}

pub(crate) fn cleanup_paths(config: Config, paths: &StorePaths) {
    for root in &paths.0 {
        cleanup_at(root, config);
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[derive(Clone, Copy)]
pub(crate) enum Selection {
    Both,
    Stdout,
    Stderr,
}

pub(crate) fn replay(id: &str, selection: Selection, tail: Option<u64>) -> io::Result<u64> {
    replay_at_to(
        &StorePaths::from_env()?,
        id,
        selection,
        tail,
        &mut io::stdout(),
        &mut io::stderr(),
    )
}

pub(crate) fn replay_at_to(
    paths: &StorePaths,
    id: &str,
    selection: Selection,
    tail: Option<u64>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u64> {
    if !valid_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid raw ID",
        ));
    }
    let mut denied = None;
    for root in &paths.0 {
        for extension in ["raw", "part"] {
            let path = root.join(format!("{id}.{extension}"));
            match File::open(&path) {
                Ok(file) => return replay_file(file, &path, selection, tail, stdout, stderr),
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound
                        || error.raw_os_error() == Some(nix::libc::ENOTDIR) => {}
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    denied = Some(error)
                }
                Err(error) => return Err(error),
            }
        }
    }
    Err(denied.unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "raw ID not found")))
}

fn replay_file(
    mut file: File,
    path: &Path,
    selection: Selection,
    tail: Option<u64>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file()
        || metadata.uid() != nix::unistd::getuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe raw file",
        ));
    }
    let mut slots = [[0u8; HEADER_SLOT]; 2];
    for slot in &mut slots {
        file.read_exact(slot)?;
    }
    let header = slots
        .iter()
        .filter_map(DiskHeader::decode)
        .max_by_key(|header| header.generation)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid raw header"))?;
    let capacity = header.capacity;
    let state = header.state;
    if capacity == 0
        || capacity > 32 * 1024 * 1024 - HEADER - JOURNAL
        || state.start >= capacity
        || state.end >= capacity
        || state.used > capacity
        || metadata.len() != (HEADER + capacity + JOURNAL) as u64
        || (state.start + state.used) % capacity != state.end
        || (header.active
            && (header.journal_at >= capacity
                || header.journal_len == 0
                || header.journal_len > JOURNAL))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid raw bounds",
        ));
    }
    let journal = if header.active {
        let mut bytes = vec![0; header.journal_len];
        file.seek(SeekFrom::Start((HEADER + capacity) as u64))?;
        file.read_exact(&mut bytes)?;
        Some(JournalView {
            at: header.journal_at,
            bytes,
        })
    } else {
        None
    };
    let mut total = 0u64;
    walk(
        &mut file,
        capacity,
        state.start,
        state.used,
        journal.as_ref(),
        |stream, bytes| {
            if selected(selection, stream) {
                total += bytes.len() as u64;
            }
            Ok(())
        },
    )?;
    let mut skip = total.saturating_sub(tail.unwrap_or(total));
    walk(
        &mut file,
        capacity,
        state.start,
        state.used,
        journal.as_ref(),
        |stream, bytes| {
            if !selected(selection, stream) {
                return Ok(());
            }
            let offset = usize::try_from(skip.min(bytes.len() as u64)).unwrap();
            skip -= offset as u64;
            if offset < bytes.len() {
                match selection {
                    Selection::Both if stream == Stream::Stderr => {
                        stderr.write_all(&bytes[offset..])?;
                        stderr.flush()?;
                    }
                    _ => {
                        stdout.write_all(&bytes[offset..])?;
                        stdout.flush()?;
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok(state.dropped)
}

struct JournalView {
    at: usize,
    bytes: Vec<u8>,
}

fn selected(selection: Selection, stream: Stream) -> bool {
    matches!(selection, Selection::Both)
        || matches!(
            (selection, stream),
            (Selection::Stdout, Stream::Stdout) | (Selection::Stderr, Stream::Stderr)
        )
}

fn walk(
    file: &mut File,
    capacity: usize,
    start: usize,
    used: usize,
    journal: Option<&JournalView>,
    mut visit: impl FnMut(Stream, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    let mut position = start;
    let mut remaining = used;
    let mut bytes = [0u8; CHUNK];
    while remaining > 0 {
        if remaining < RECORD_HEADER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "short raw record",
            ));
        }
        let mut header = [0u8; RECORD_HEADER];
        read_ring_view(file, capacity, position, &mut header, journal)?;
        let stream = Stream::from_byte(header[0])?;
        let length = u32::from_le_bytes(header[1..5].try_into().unwrap()) as usize;
        if length == 0 || length > CHUNK || RECORD_HEADER + length > remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid raw record",
            ));
        }
        position = (position + RECORD_HEADER) % capacity;
        read_ring_view(file, capacity, position, &mut bytes[..length], journal)?;
        visit(stream, &bytes[..length])?;
        position = (position + length) % capacity;
        remaining -= RECORD_HEADER + length;
    }
    Ok(())
}

fn read_ring_view(
    file: &mut File,
    capacity: usize,
    at: usize,
    bytes: &mut [u8],
    journal: Option<&JournalView>,
) -> io::Result<()> {
    read_ring(file, capacity, at, bytes)?;
    if let Some(journal) = journal {
        let mut distance = (at + capacity - journal.at) % capacity;
        for byte in bytes {
            if distance < journal.bytes.len() {
                *byte = journal.bytes[distance];
            }
            distance += 1;
            if distance == capacity {
                distance = 0;
            }
        }
    }
    Ok(())
}

fn read_ring(file: &mut File, capacity: usize, at: usize, bytes: &mut [u8]) -> io::Result<()> {
    let first = bytes.len().min(capacity - at);
    file.seek(SeekFrom::Start((HEADER + at) as u64))?;
    file.read_exact(&mut bytes[..first])?;
    if first < bytes.len() {
        file.seek(SeekFrom::Start(HEADER as u64))?;
        file.read_exact(&mut bytes[first..])?;
    }
    Ok(())
}
