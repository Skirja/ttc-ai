//! Local, bounded I/O and same-directory atomic staging.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

pub(super) type Result<T> = std::result::Result<T, String>;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identity(u64, u64);

pub(super) fn identity(path: &Path) -> Result<Option<Identity>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && meta.uid() == nix::unistd::geteuid().as_raw() => {
            Ok(Some(Identity(meta.dev(), meta.ino())))
        }
        Ok(_) => Err(format!("{} bukan regular file milik user", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

pub(super) fn open_read(path: &Path) -> Result<File> {
    identity(path)?.ok_or_else(|| format!("{} tidak ada", path.display()))?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let meta = file.metadata().map_err(|error| error.to_string())?;
    if !meta.is_file() || meta.uid() != nix::unistd::geteuid().as_raw() {
        return Err("File bukan regular file milik user".into());
    }
    Ok(file)
}

pub(super) fn read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    open_read(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > maximum {
        return Err(format!("{} terlalu besar", path.display()));
    }
    Ok(bytes)
}

pub(super) fn hash(path: &Path) -> Result<String> {
    let mut file = open_read(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(super) fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Sinkronisasi {}: {error}", path.display()))
}

pub(super) fn directory(path: &Path) -> Result<()> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            directory(parent)?;
        }
        match fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    }
    let meta = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(format!("{} bukan directory regular", path.display()));
    }
    Ok(())
}

pub(super) fn owned_directory(path: &Path) -> Result<()> {
    directory(path)?;
    let meta = fs::metadata(path).map_err(|error| error.to_string())?;
    if meta.uid() != nix::unistd::geteuid().as_raw() || meta.mode() & 0o022 != 0 {
        return Err(format!(
            "{} harus milik user dan tidak writable oleh user lain",
            path.display()
        ));
    }
    Ok(())
}

pub(super) struct Lock {
    _file: File,
}

impl Lock {
    pub fn acquire(path: &Path) -> Result<Self> {
        identity(path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path)
            .map_err(|error| format!("Lock {}: {error}", path.display()))?;
        let meta = file.metadata().map_err(|error| error.to_string())?;
        if !meta.is_file()
            || meta.uid() != nix::unistd::geteuid().as_raw()
            || meta.mode() & 0o077 != 0
        {
            return Err("Lock bukan private regular file milik user".into());
        }
        file.lock().map_err(|error| error.to_string())?;
        Ok(Self { _file: file })
    }
}

pub(super) struct Stage {
    pub path: PathBuf,
    id: Identity,
    clean: bool,
}

impl Stage {
    fn create(directory: &Path, purpose: &str, mode: u32) -> Result<(Self, File)> {
        for _ in 0..128 {
            let path = directory.join(format!(
                ".ttc-{}-{}-{purpose}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .open(&path)
            {
                Ok(file) => {
                    let meta = file.metadata().map_err(|error| error.to_string())?;
                    return Ok((
                        Self {
                            path,
                            id: Identity(meta.dev(), meta.ino()),
                            clean: true,
                        },
                        file,
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("Tidak dapat membuat file staging unik".into())
    }

    pub fn bytes(directory: &Path, purpose: &str, bytes: &[u8], mode: u32) -> Result<Self> {
        let (stage, mut file) = Self::create(directory, purpose, mode)?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        Ok(stage)
    }

    pub fn copy(directory: &Path, purpose: &str, source: &Path, mode: u32) -> Result<Self> {
        let mut source = open_read(source)?;
        let (stage, mut file) = Self::create(directory, purpose, mode)?;
        std::io::copy(&mut source, &mut file).map_err(|error| error.to_string())?;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        Ok(stage)
    }

    pub fn id(&self) -> Identity {
        self.id
    }

    pub fn replace(&mut self, destination: &Path) -> Result<()> {
        if identity(&self.path)? != Some(self.id) {
            return Err("Staging berubah".into());
        }
        fs::rename(&self.path, destination).map_err(|error| error.to_string())?;
        self.clean = false;
        sync_directory(destination.parent().ok_or("Parent path tidak ada")?)
    }

    // Publishing a fresh path must not overwrite a concurrently created file.
    pub fn publish(&self, destination: &Path) -> Result<()> {
        if identity(&self.path)? != Some(self.id) {
            return Err("Staging berubah".into());
        }
        fs::hard_link(&self.path, destination).map_err(|error| error.to_string())?;
        sync_directory(destination.parent().ok_or("Parent path tidak ada")?)
    }

    pub fn keep(&mut self) {
        self.clean = false;
    }

    pub fn adopt(&mut self, id: Identity) {
        self.id = id;
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if self.clean && identity(&self.path).ok().flatten() == Some(self.id) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(super) fn remove_owned(path: &Path, expected: Identity) -> Result<()> {
    if identity(path)? != Some(expected) {
        return Err(format!("{} berubah", path.display()));
    }
    fs::remove_file(path).map_err(|error| error.to_string())?;
    sync_directory(path.parent().ok_or("Parent path tidak ada")?)
}
