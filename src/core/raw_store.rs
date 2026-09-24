//! Bounded, tagged capture of the original output bytes.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::config::Config;

const HEADER: usize = 64;
const RECORD_HEADER: usize = 5;
const CHUNK: usize = 32 * 1024;
const MAGIC: &[u8; 8] = b"TTCRAW1\0";

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
    backend: Backend,
    pending_path: Option<PathBuf>,
    final_path: Option<PathBuf>,
    id: Option<String>,
}

impl Capture {
    pub(crate) fn new(config: Config, roots: StorePaths) -> Self {
        let capacity = config.max_bytes() - HEADER;
        Self {
            roots,
            capacity,
            start: 0,
            end: 0,
            used: 0,
            dropped: 0,
            backend: Backend::Memory(vec![0; capacity]),
            pending_path: None,
            final_path: None,
            id: None,
        }
    }

    pub(crate) fn record(&mut self, stream: Stream, bytes: &[u8]) -> io::Result<()> {
        for chunk in bytes.chunks(CHUNK) {
            if chunk.is_empty() {
                continue;
            }
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
            let mut header = [0; RECORD_HEADER];
            header[0] = stream as u8;
            header[1..5].copy_from_slice(&(chunk.len() as u32).to_le_bytes());
            self.write_at(self.end, &header)?;
            self.end = (self.end + RECORD_HEADER) % self.capacity;
            self.write_at(self.end, chunk)?;
            self.end = (self.end + chunk.len()) % self.capacity;
            self.used += needed;
            if matches!(self.backend, Backend::File(_)) {
                self.write_header()?;
            }
        }
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
            let header = self.header_bytes();
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
        self.write_header()?;
        if let Backend::File(file) = &mut self.backend {
            file.flush()?;
        }
        fs::rename(&pending, self.final_path.as_ref().unwrap())?;
        self.pending_path = None;
        Ok(self.id.as_deref())
    }

    fn header_bytes(&self) -> [u8; HEADER] {
        let mut header = [0; HEADER];
        header[..8].copy_from_slice(MAGIC);
        for (index, value) in [self.capacity, self.start, self.end, self.used]
            .into_iter()
            .enumerate()
        {
            header[8 + index * 8..16 + index * 8].copy_from_slice(&(value as u64).to_le_bytes());
        }
        header[40..48].copy_from_slice(&self.dropped.to_le_bytes());
        header
    }

    fn write_header(&mut self) -> io::Result<()> {
        let header = self.header_bytes();
        if let Backend::File(file) = &mut self.backend {
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&header)?;
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

impl Drop for Capture {
    fn drop(&mut self) {
        if let Some(path) = &self.pending_path {
            let _ = fs::remove_file(path);
        }
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
        Ok(Self([
            state.join("ttc/runs"),
            std::env::temp_dir().join(format!("ttc-{uid}/runs")),
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
    replay_at(&StorePaths::from_env()?, id, selection, tail)
}

pub(crate) fn replay_at(
    paths: &StorePaths,
    id: &str,
    selection: Selection,
    tail: Option<u64>,
) -> io::Result<u64> {
    if !valid_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid raw ID",
        ));
    }
    for root in &paths.0 {
        let path = root.join(format!("{id}.raw"));
        match File::open(&path) {
            Ok(file) => return replay_file(file, &path, selection, tail),
            Err(error)
                if error.kind() == io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(nix::libc::ENOTDIR) => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "raw ID not found"))
}

fn replay_file(
    mut file: File,
    path: &Path,
    selection: Selection,
    tail: Option<u64>,
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
    let mut header = [0u8; HEADER];
    file.read_exact(&mut header)?;
    if &header[..8] != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid raw header",
        ));
    }
    let number = |range: std::ops::Range<usize>| -> usize {
        u64::from_le_bytes(header[range].try_into().unwrap()) as usize
    };
    let capacity = number(8..16);
    let start = number(16..24);
    let end = number(24..32);
    let used = number(32..40);
    let dropped = u64::from_le_bytes(header[40..48].try_into().unwrap());
    if capacity == 0
        || capacity > 32 * 1024 * 1024 - HEADER
        || start >= capacity
        || end >= capacity
        || used > capacity
        || metadata.len() != (HEADER + capacity) as u64
        || (start + used) % capacity != end
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid raw bounds",
        ));
    }
    let mut total = 0u64;
    walk(&mut file, capacity, start, used, |stream, bytes| {
        if selected(selection, stream) {
            total += bytes.len() as u64;
        }
        Ok(())
    })?;
    let mut skip = total.saturating_sub(tail.unwrap_or(total));
    walk(&mut file, capacity, start, used, |stream, bytes| {
        if !selected(selection, stream) {
            return Ok(());
        }
        let offset = usize::try_from(skip.min(bytes.len() as u64)).unwrap();
        skip -= offset as u64;
        if offset < bytes.len() {
            match selection {
                Selection::Both if stream == Stream::Stderr => {
                    let mut stderr = io::stderr();
                    stderr.write_all(&bytes[offset..])?;
                    stderr.flush()?;
                }
                _ => {
                    let mut stdout = io::stdout();
                    stdout.write_all(&bytes[offset..])?;
                    stdout.flush()?;
                }
            }
        }
        Ok(())
    })?;
    Ok(dropped)
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
        read_ring(file, capacity, position, &mut header)?;
        let stream = Stream::from_byte(header[0])?;
        let length = u32::from_le_bytes(header[1..5].try_into().unwrap()) as usize;
        if length == 0 || length > CHUNK || RECORD_HEADER + length > remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid raw record",
            ));
        }
        position = (position + RECORD_HEADER) % capacity;
        read_ring(file, capacity, position, &mut bytes[..length])?;
        visit(stream, &bytes[..length])?;
        position = (position + length) % capacity;
        remaining -= RECORD_HEADER + length;
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
