//! Local, bounded I/O and same-directory atomic staging.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
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

pub(super) fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// Archives must stay on the target filesystem, in a proven TTC namespace.
// A pre-existing private directory alone is not evidence of TTC ownership.
pub(super) fn workspace(path: &Path, installed: &Path) -> Result<()> {
    let receipt = format!("ttc-distribution-workspace-v1\n{}\n", installed.display());
    let created = match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error.to_string()),
    };
    owned_directory(path)?;
    let meta = fs::metadata(path).map_err(|error| error.to_string())?;
    let parent = path.parent().ok_or("Parent workspace tidak ada")?;
    if meta.mode() & 0o077 != 0
        || meta.dev()
            != fs::metadata(parent)
                .map_err(|error| error.to_string())?
                .dev()
    {
        return Err("Workspace distribusi harus privat dan pada filesystem target".into());
    }
    let owner = path.join("owner");
    if created {
        Stage::bytes(path, "owner", receipt.as_bytes(), 0o600)?.publish(&owner)?;
        sync_directory(parent)?;
    }
    if read(&owner, 16 * 1024)? != receipt.as_bytes()
        || fs::metadata(&owner)
            .map_err(|error| error.to_string())?
            .mode()
            & 0o077
            != 0
    {
        return Err("Receipt ownership workspace distribusi tidak valid".into());
    }
    Ok(())
}

pub(super) fn sync_pair(left: &Path, right: &Path) -> Result<()> {
    let left = left.parent().ok_or("Parent archive tidak ada")?;
    let right = right.parent().ok_or("Parent target tidak ada")?;
    sync_directory(left)?;
    if left != right {
        sync_directory(right)?;
    }
    Ok(())
}

pub(super) fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("Sinkronisasi {}: {error}", path.display()))
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(super) fn exchange(left: &Path, right: &Path) -> Result<()> {
    use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};
    renameat2(
        AT_FDCWD,
        left,
        AT_FDCWD,
        right,
        RenameFlags::RENAME_EXCHANGE,
    )
    .map_err(|error| format!("Pertukaran file atomik gagal: {error}"))
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub(super) fn exchange(_left: &Path, _right: &Path) -> Result<()> {
    Err("Pertukaran file atomik memerlukan Linux GNU".into())
}

// Capture the entry at the instant of rename; never unlink an editor's save
// based on a preceding identity/content check.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(super) fn capture(path: &Path) -> Result<PathBuf> {
    capture_in(
        path,
        path.parent().ok_or("Parent path tidak ada")?,
        "bashrc-recovery",
    )
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(super) fn capture_in(path: &Path, directory: &Path, purpose: &str) -> Result<PathBuf> {
    use nix::errno::Errno;
    use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};
    for _ in 0..128 {
        let recovery = directory.join(format!(
            ".ttc-{}-{}-{purpose}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match renameat2(
            AT_FDCWD,
            path,
            AT_FDCWD,
            &recovery,
            RenameFlags::RENAME_NOREPLACE,
        ) {
            Ok(()) => return Ok(recovery),
            Err(Errno::EEXIST) => {}
            Err(error) => return Err(format!("Capture file atomik gagal: {error}")),
        }
    }
    Err("Tidak dapat membuat lokasi recovery unik".into())
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub(super) fn capture(_path: &Path) -> Result<PathBuf> {
    Err("Capture config atomik memerlukan Linux GNU".into())
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
pub(super) fn capture_in(_path: &Path, _directory: &Path, _purpose: &str) -> Result<PathBuf> {
    Err("Capture file atomik memerlukan Linux GNU".into())
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

    pub fn exchange(&mut self, destination: &Path) -> Result<()> {
        if identity(&self.path)? != Some(self.id) {
            return Err("Staging berubah".into());
        }
        exchange(&self.path, destination)?;
        // The stage now contains the actual displaced user entry, including
        // a racing symlink. Never let Drop delete it, even after a sync error.
        self.keep();
        Ok(())
    }

    // Publishing a fresh path must not overwrite a concurrently created file.
    pub fn publish(&self, destination: &Path) -> Result<()> {
        if identity(&self.path)? != Some(self.id) {
            return Err("Staging berubah".into());
        }
        fs::hard_link(&self.path, destination).map_err(|error| error.to_string())?;
        sync_pair(&self.path, destination)
    }

    pub fn keep(&mut self) {
        self.clean = false;
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        if self.clean && identity(&self.path).ok().flatten() == Some(self.id) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn move_noreplace(from: &Path, to: &Path) -> Result<()> {
    use nix::fcntl::{AT_FDCWD, RenameFlags, renameat2};
    renameat2(AT_FDCWD, from, AT_FDCWD, to, RenameFlags::RENAME_NOREPLACE)
        .map_err(|error| error.to_string())
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn move_noreplace(_from: &Path, _to: &Path) -> Result<()> {
    Err("Recovery atomik memerlukan Linux GNU".into())
}

pub(super) fn remove_owned(path: &Path, expected: Identity) -> Result<()> {
    remove_owned_with(path, expected, || Ok(()))
}

fn remove_owned_with(
    path: &Path,
    expected: Identity,
    before_capture: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if identity(path)? != Some(expected) {
        return Err(format!("{} berubah", path.display()));
    }
    before_capture()?;
    let recovery = capture_in(
        path,
        path.parent().ok_or("Parent marker tidak ada")?,
        "marker-recovery",
    )?;
    if identity(&recovery).ok().flatten() != Some(expected) {
        // Restore without clobbering a newly-created marker. Keep the actual
        // entry in recovery even when restoration cannot be completed.
        move_noreplace(&recovery, path)
            .map_err(|error| format!("Marker recovery {}: {error}", recovery.display()))?;
        sync_pair(&recovery, path)?;
        return Err(format!(
            "Marker berubah; entry dipulihkan di {}",
            path.display()
        ));
    }
    sync_pair(&recovery, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_capture_restores_raced_entries_without_following_or_overwriting() {
        for kind in ["file", "symlink", "directory"] {
            let root = std::env::temp_dir().join(format!(
                "ttc-marker-race-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let marker = root.join("pending");
            fs::write(&marker, b"owned marker").unwrap();
            let id = identity(&marker).unwrap().unwrap();
            let foreign = root.join("foreign");
            fs::write(&foreign, b"user marker state").unwrap();
            assert!(
                remove_owned_with(&marker, id, || {
                    fs::remove_file(&marker).unwrap();
                    match kind {
                        "file" => fs::rename(&foreign, &marker).unwrap(),
                        "symlink" => std::os::unix::fs::symlink(&foreign, &marker).unwrap(),
                        "directory" => {
                            fs::create_dir(&marker).unwrap();
                            fs::write(marker.join("user-data"), b"user marker state").unwrap();
                        }
                        _ => unreachable!(),
                    }
                    Ok(())
                })
                .is_err()
            );
            match kind {
                "file" => assert_eq!(fs::read(&marker).unwrap(), b"user marker state"),
                "symlink" => {
                    assert_eq!(fs::read_link(&marker).unwrap(), foreign);
                    assert_eq!(fs::read(&foreign).unwrap(), b"user marker state");
                }
                "directory" => assert_eq!(
                    fs::read(marker.join("user-data")).unwrap(),
                    b"user marker state"
                ),
                _ => unreachable!(),
            }
            fs::remove_dir_all(root).unwrap();
        }
    }
}
