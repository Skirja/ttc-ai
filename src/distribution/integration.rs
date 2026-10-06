//! Local config transactions exposed to adapters; no harness types enter here.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::files::{self, Identity, Lock, Result, Stage};
use super::transaction::{Expected, Removal, Replacement};
use super::{Paths, Snapshot, binary_owned, metadata};

pub(crate) const CONFIG_LIMIT: usize = 1024 * 1024;

pub(crate) struct FileSnapshot {
    pub path: PathBuf,
    pub bytes: Option<Vec<u8>>,
    mode: u32,
    identity: Option<Identity>,
}

impl FileSnapshot {
    pub fn read(path: &Path) -> Result<Self> {
        files::owned_directory(path.parent().ok_or("Parent config tidak ada")?)?;
        let identity = files::identity(path)?;
        let (bytes, mode) = if identity.is_some() {
            let mode = fs::metadata(path)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o777;
            if mode & 0o200 == 0 || mode & 0o022 != 0 {
                return Err("Config harus writable dan tidak writable oleh user lain".into());
            }
            let bytes = files::read(path, CONFIG_LIMIT)?;
            if files::identity(path)? != identity {
                return Err("Config berubah saat dibaca".into());
            }
            (Some(bytes), mode)
        } else {
            (None, 0o600)
        };
        Ok(Self {
            path: path.into(),
            bytes,
            mode,
            identity,
        })
    }

    fn expected(&self) -> Option<Expected> {
        self.identity
            .zip(self.bytes.as_ref())
            .map(|(id, bytes)| Expected {
                id,
                hash: files::hash_bytes(bytes),
            })
    }

    pub fn change(self, after: Option<Vec<u8>>) -> Change {
        Change {
            before: self,
            after,
        }
    }
}

pub(crate) struct Change {
    before: FileSnapshot,
    after: Option<Vec<u8>>,
}

pub(crate) struct Context {
    paths: Paths,
    previous: Snapshot,
    _lock: Lock,
}

enum Edit {
    Replace(Replacement),
    Remove(Removal),
}
impl Edit {
    fn apply_with(&mut self, callback: impl FnOnce() -> Result<()>) -> Result<()> {
        match self {
            Self::Replace(edit) => edit.apply_with(callback),
            Self::Remove(edit) => edit.apply_with(callback),
        }
    }
    fn rollback(&mut self) -> Result<()> {
        match self {
            Self::Replace(edit) => edit.rollback(),
            Self::Remove(edit) => edit.rollback(),
        }
    }
}

impl Context {
    pub fn begin() -> Result<Self> {
        let paths = Paths::from_env()?;
        let lock = paths.prepare()?;
        let previous = paths
            .load()?
            .ok_or("TTC belum terdaftar; jalankan installer binary terlebih dahulu")?;
        binary_owned(&paths, Some(&previous), false)?;
        Ok(Self {
            paths,
            previous,
            _lock: lock,
        })
    }
    pub fn binary(&self) -> &Path {
        &self.paths.binary
    }
    pub fn receipt(&self, name: &str) -> PathBuf {
        self.paths.data.join(format!("{name}.toml"))
    }
    pub fn active(&self, name: &str) -> bool {
        self.previous
            .meta
            .active_harnesses
            .iter()
            .any(|h| h == name)
    }

    pub fn commit(self, name: &str, active: bool, changes: Vec<Change>) -> Result<Vec<PathBuf>> {
        self.commit_with(name, active, changes, |_| Ok(()))
    }

    fn commit_with(
        self,
        name: &str,
        active: bool,
        mut changes: Vec<Change>,
        mut before_edit: impl FnMut(usize) -> Result<()>,
    ) -> Result<Vec<PathBuf>> {
        let mut meta = self.previous.meta.clone();
        meta.active_harnesses.retain(|h| h != name);
        if active {
            meta.active_harnesses.push(name.to_owned());
        }
        changes.push(
            FileSnapshot {
                path: self.paths.manifest.clone(),
                bytes: Some(self.previous.bytes.clone()),
                mode: 0o600,
                identity: Some(self.previous.id),
            }
            .change(Some(meta.encode()?)),
        );
        let mut edits = Vec::new();
        let mut backups = Vec::new();
        for change in changes {
            if change.before.bytes == change.after {
                continue;
            }
            let parent = change
                .before
                .path
                .parent()
                .ok_or("Parent config tidak ada")?;
            let workspace = parent.join(".ttc-distribution");
            files::workspace(&workspace, &self.paths.binary)?;
            let expected = change.before.expected();
            if let Some(after) = change.after {
                if after.len() > CONFIG_LIMIT {
                    return Err("Config hasil edit melebihi 1 MiB".into());
                }
                let stage =
                    Stage::bytes(&workspace, "integration-backup", &after, change.before.mode)?;
                if expected.is_some() {
                    backups.push(stage.path.clone());
                }
                edits.push(Edit::Replace(Replacement::new(
                    stage,
                    &change.before.path,
                    expected,
                )?));
            } else if let Some(expected) = expected {
                edits.push(Edit::Remove(Removal::new(
                    &change.before.path,
                    &workspace,
                    expected,
                )));
            }
        }
        if edits.is_empty() {
            return Ok(Vec::new());
        }
        self.paths.unchanged(Some(&self.previous))?;
        binary_owned(&self.paths, Some(&self.previous), false)?;
        let marker = self.paths.marker("integration")?;
        let result = edits
            .iter_mut()
            .enumerate()
            .try_for_each(|(index, edit)| edit.apply_with(|| before_edit(index)));
        if let Err(error) = result {
            let mut recovery = Vec::new();
            for edit in edits.iter_mut().rev() {
                if let Err(error) = edit.rollback() {
                    recovery.push(error);
                }
            }
            if recovery.is_empty() {
                files::remove_owned(&self.paths.pending, marker)?;
            }
            return Err(format!(
                "{error}; {}",
                if recovery.is_empty() {
                    "rollback selesai".to_owned()
                } else {
                    format!("{}; marker recovery dipertahankan", recovery.join("; "))
                }
            ));
        }
        files::remove_owned(&self.paths.pending, marker)?;
        Ok(backups)
    }
}

pub(crate) fn registered() -> Result<bool> {
    let paths = Paths::from_env()?;
    Ok(files::identity(&paths.manifest)?.is_some())
}

pub(crate) fn text_path(path: &Path) -> Result<String> {
    metadata::text_path(path)
}

#[cfg(test)]
mod tests {
    use super::super::metadata::Metadata;
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "ttc-m10-txn-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn paths(&self) -> Paths {
            Self::paths_at(&self.0)
        }
        fn paths_at(root: &Path) -> Paths {
            Paths {
                home: root.into(),
                data: root.join("data"),
                binary: root.join("bin/ttc"),
                manifest: root.join("data/install.toml"),
                pending: root.join("data/install.pending"),
            }
        }
        fn initialize(&self) {
            fs::create_dir(self.0.join("bin")).unwrap();
            fs::create_dir(self.0.join("data")).unwrap();
            fs::write(self.paths().binary, b"owned binary").unwrap();
            let meta = Metadata {
                version: "0.1.0".into(),
                sha256: files::hash_bytes(b"owned binary"),
                installed_path: self.paths().binary.to_str().unwrap().into(),
                active_harnesses: Vec::new(),
                path: None,
            };
            fs::write(self.paths().manifest, meta.encode().unwrap()).unwrap();
        }
        fn context(&self) -> Context {
            Self::context_at(&self.0)
        }
        fn context_at(root: &Path) -> Context {
            let paths = Self::paths_at(root);
            let lock = paths.prepare().unwrap();
            let previous = paths.load().unwrap().unwrap();
            Context {
                paths,
                previous,
                _lock: lock,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn later_metadata_failure_rolls_back_config_and_receipt() {
        let fixture = Fixture::new();
        fixture.initialize();
        let config = fixture.0.join("config.toml");
        fs::write(&config, b"original").unwrap();
        let context = fixture.context();
        let receipt = context.receipt("codex");
        let changes = vec![
            FileSnapshot::read(&config)
                .unwrap()
                .change(Some(b"edited".to_vec())),
            FileSnapshot::read(&receipt)
                .unwrap()
                .change(Some(b"receipt".to_vec())),
        ];
        assert!(
            context
                .commit_with("codex", true, changes, |index| if index == 2 {
                    Err("injected metadata failure".into())
                } else {
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(fs::read(config).unwrap(), b"original");
        assert!(!receipt.exists());
        assert!(!fixture.paths().pending.exists());
        assert!(
            fixture
                .paths()
                .load()
                .unwrap()
                .unwrap()
                .meta
                .active_harnesses
                .is_empty()
        );
    }

    #[test]
    fn late_config_save_is_archived_and_blocks_following_mutation() {
        for rename in [false, true] {
            let fixture = Fixture::new();
            fixture.initialize();
            let config = fixture.0.join("config.toml");
            fs::write(&config, b"original").unwrap();
            let context = fixture.context();
            let change = FileSnapshot::read(&config)
                .unwrap()
                .change(Some(b"ttc edit".to_vec()));
            let result = context.commit_with("codex", true, vec![change], |index| {
                if index == 0 {
                    if rename {
                        let save = fixture.0.join("save");
                        fs::write(&save, b"user late save").unwrap();
                        fs::rename(save, &config).unwrap();
                    } else {
                        fs::write(&config, b"user late save").unwrap();
                    }
                }
                Ok(())
            });
            assert!(result.is_err());
            assert!(fixture.paths().pending.exists());
            assert!(fixture.paths().prepare().is_err());
            assert_eq!(fs::read(&config).unwrap(), b"ttc edit");
            assert!(
                fs::read_dir(fixture.0.join(".ttc-distribution"))
                    .unwrap()
                    .any(|entry| fs::read(entry.unwrap().path())
                        .is_ok_and(|bytes| bytes == b"user late save"))
            );
        }
    }

    #[test]
    fn sigkill_after_config_commit_retains_backup_and_pending_marker() {
        const WORKER: &str = "TTC_M10_TRANSACTION_WORKER";
        if let Some(root) = std::env::var_os(WORKER) {
            let root = PathBuf::from(root);
            let config = root.join("config.toml");
            let context = Fixture::context_at(&root);
            let receipt = context.receipt("codex");
            let changes = vec![
                FileSnapshot::read(&config)
                    .unwrap()
                    .change(Some(b"edited".to_vec())),
                FileSnapshot::read(&receipt)
                    .unwrap()
                    .change(Some(b"receipt".to_vec())),
            ];
            let _ = context.commit_with("codex", true, changes, |index| {
                if index == 1 {
                    nix::sys::signal::kill(
                        nix::unistd::getpid(),
                        nix::sys::signal::Signal::SIGKILL,
                    )
                    .unwrap();
                }
                Ok(())
            });
            panic!("worker must be killed");
        }
        let fixture = Fixture::new();
        fixture.initialize();
        fs::write(fixture.0.join("config.toml"), b"original").unwrap();
        let status=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","distribution::integration::tests::sigkill_after_config_commit_retains_backup_and_pending_marker"]).env(WORKER,&fixture.0).status().unwrap();
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(9));
        assert!(fixture.paths().pending.exists());
        assert!(fixture.paths().prepare().is_err());
        assert_eq!(fs::read(fixture.0.join("config.toml")).unwrap(), b"edited");
        assert!(
            fs::read_dir(fixture.0.join(".ttc-distribution"))
                .unwrap()
                .any(
                    |entry| fs::read(entry.unwrap().path()).is_ok_and(|bytes| bytes == b"original")
                )
        );
    }
}
