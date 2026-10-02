//! Local installation management, independent from execution and harnesses.

mod files;
mod metadata;
mod path;
mod transaction;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use files::{Identity, Result, Stage};
use metadata::Metadata;
use transaction::{Expected, Removal, Replacement};

struct Paths {
    home: PathBuf,
    data: PathBuf,
    binary: PathBuf,
    manifest: PathBuf,
    pending: PathBuf,
}

impl Paths {
    fn from_env() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or("HOME tidak tersedia")?;
        if !home.is_absolute() {
            return Err("HOME harus absolute".into());
        }
        let home = fs::canonicalize(home).map_err(|error| error.to_string())?;
        let data = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        if !data.is_absolute() {
            return Err("XDG_DATA_HOME harus absolute".into());
        }
        metadata::text_path(&home)?;
        metadata::text_path(&data)?;
        let data = data.join("ttc");
        Ok(Self {
            binary: home.join(".local/bin/ttc"),
            manifest: data.join("install.toml"),
            pending: data.join("install.pending"),
            home,
            data,
        })
    }

    fn prepare(&self) -> Result<files::Lock> {
        files::owned_directory(&self.home)?;
        files::owned_directory(&self.data)?;
        files::owned_directory(self.binary.parent().ok_or("Parent binary tidak ada")?)?;
        let lock = files::Lock::acquire(&self.data.join("install.lock"))?;
        if fs::symlink_metadata(&self.pending).is_ok() {
            return Err(format!(
                "Transaksi sebelumnya terputus: {}; verifikasi file sebelum recovery manual",
                self.pending.display()
            ));
        }
        files::workspace(&self.binary_workspace(), &self.binary)?;
        files::workspace(&self.metadata_workspace(), &self.binary)?;
        Ok(lock)
    }

    fn binary_workspace(&self) -> PathBuf {
        self.binary
            .parent()
            .expect("absolute binary path")
            .join(".ttc-distribution")
    }

    fn metadata_workspace(&self) -> PathBuf {
        self.data.join(".ttc-distribution")
    }

    fn load(&self) -> Result<Option<Snapshot>> {
        let Some(id) = files::identity(&self.manifest)? else {
            return Ok(None);
        };
        let bytes = files::read(&self.manifest, metadata::MAX_METADATA)?;
        let meta = Metadata::parse(
            &bytes,
            &self.binary,
            &self.home.join(".bashrc"),
            path::BLOCK,
        )?;
        Ok(Some(Snapshot { id, bytes, meta }))
    }

    fn marker(&self, operation: &str) -> Result<Identity> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.pending)
            .map_err(|error| error.to_string())?;
        writeln!(file, "operation = {operation:?}")
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        let id = files::identity(&self.pending)?.ok_or("Marker tidak ada")?;
        files::sync_directory(&self.data)?;
        Ok(id)
    }

    fn unchanged(&self, previous: Option<&Snapshot>) -> Result<()> {
        match previous {
            Some(previous)
                if files::identity(&self.manifest)? == Some(previous.id)
                    && files::read(&self.manifest, metadata::MAX_METADATA)? == previous.bytes =>
            {
                Ok(())
            }
            None if files::identity(&self.manifest)?.is_none() => Ok(()),
            _ => Err("Metadata berubah selama transaksi".into()),
        }
    }
}

struct Snapshot {
    id: Identity,
    bytes: Vec<u8>,
    meta: Metadata,
}

fn binary_owned(
    paths: &Paths,
    previous: Option<&Snapshot>,
    allow_missing: bool,
) -> Result<Option<Identity>> {
    let id = files::identity(&paths.binary)?;
    match (previous, id) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(
            "Binary existing tidak terbukti dimiliki TTC; instalasi manual tidak diadopsi".into(),
        ),
        (Some(_), None) if allow_missing => Ok(None),
        (Some(_), None) => Err("Binary terdaftar tidak ada; metadata dipertahankan".into()),
        (Some(previous), Some(id)) if files::hash(&paths.binary)? == previous.meta.sha256 => {
            Ok(Some(id))
        }
        _ => Err("Checksum binary existing berbeda; file dipertahankan".into()),
    }
}

pub(crate) fn install(expected: &str) -> ExitCode {
    let result = (|| {
        if !metadata::checksum(expected) {
            return Err("Checksum instalasi tidak valid".into());
        }
        let paths = Paths::from_env()?;
        let source = std::env::current_exe().map_err(|error| error.to_string())?;
        install_from(&paths, &source, expected, || Ok(()))?;
        println!(
            "TTC {} terpasang di {}",
            env!("CARGO_PKG_VERSION"),
            paths.binary.display()
        );
        println!("Jalankan {} --help", path::quote(&paths.binary));
        Ok(())
    })();
    status(result)
}

fn install_from(
    paths: &Paths,
    source: &Path,
    expected: &str,
    before_metadata: impl FnOnce() -> Result<()>,
) -> Result<()> {
    install_with(paths, source, expected, || Ok(()), before_metadata)
}

fn install_with(
    paths: &Paths,
    source: &Path,
    expected: &str,
    before_binary: impl FnOnce() -> Result<()>,
    before_metadata: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let _lock = paths.prepare()?;
    let previous = paths.load()?;
    let previous_id = binary_owned(paths, previous.as_ref(), false)?;
    let image = Stage::copy(&paths.binary_workspace(), "image", source, 0o755)?;
    if files::hash(&image.path)? != expected {
        return Err("Checksum candidate tidak cocok".into());
    }
    let binary_before = previous
        .as_ref()
        .zip(previous_id)
        .map(|(snapshot, id)| Expected {
            id,
            hash: snapshot.meta.sha256.clone(),
        });
    let mut image = Replacement::new(image, &paths.binary, binary_before)?;
    let mut meta = Metadata {
        version: env!("CARGO_PKG_VERSION").into(),
        sha256: expected.into(),
        installed_path: metadata::text_path(&paths.binary)?,
        active_harnesses: previous
            .as_ref()
            .map(|p| p.meta.active_harnesses.clone())
            .unwrap_or_default(),
        path: previous.as_ref().and_then(|p| p.meta.path.clone()),
    };
    let manifest = Stage::bytes(
        &paths.metadata_workspace(),
        "manifest",
        &meta.encode()?,
        0o600,
    )?;
    let metadata_before = previous.as_ref().map(|snapshot| Expected {
        id: snapshot.id,
        hash: files::hash_bytes(&snapshot.bytes),
    });
    let mut manifest = Replacement::new(manifest, &paths.manifest, metadata_before)?;
    paths.unchanged(previous.as_ref())?;
    if binary_owned(paths, previous.as_ref(), false)? != previous_id {
        return Err("Binary berubah sebelum replacement".into());
    }
    let marker = paths.marker("install")?;
    let committed = image
        .apply_with(before_binary)
        .and_then(|()| manifest.apply_with(before_metadata));
    if let Err(error) = committed {
        let recovery = image.rollback().and_then(|()| manifest.rollback());
        if recovery.is_ok() {
            files::remove_owned(&paths.pending, marker)?;
            return Err(error);
        }
        return Err(format!(
            "{error}; {}; marker dipertahankan",
            recovery.unwrap_err()
        ));
    }
    let path_result = update_path(paths, &mut meta);
    if let Err(PathFailure::Ambiguous(error)) = &path_result {
        return Err(format!("{error}; marker transaksi dipertahankan"));
    }
    if let Err(PathFailure::Manual(error)) = path_result {
        eprintln!("ttc: PATH otomatis gagal: {error}");
        println!("PATH manual: export PATH=\"$HOME/.local/bin${{PATH:+:$PATH}}\"");
    } else if meta.path.is_some() {
        println!(
            "Reload Bash: source {} atau buka shell baru",
            path::quote(&paths.home.join(".bashrc"))
        );
    }
    files::remove_owned(&paths.pending, marker)
}

enum PathFailure {
    Manual(String),
    Ambiguous(String),
}

fn update_path(paths: &Paths, meta: &mut Metadata) -> std::result::Result<(), PathFailure> {
    let directory = paths
        .binary
        .parent()
        .ok_or_else(|| PathFailure::Manual("Parent binary tidak ada".into()))?;
    let Some((mut edit, ownership)) =
        path::append(&paths.home.join(".bashrc"), directory, meta.path.as_ref())
            .map_err(PathFailure::Manual)?
    else {
        return Ok(());
    };
    let previous_bytes = meta.encode().map_err(PathFailure::Manual)?;
    let previous_id = files::identity(&paths.manifest)
        .map_err(PathFailure::Ambiguous)?
        .ok_or_else(|| PathFailure::Ambiguous("Metadata instalasi tidak ada".into()))?;
    if files::read(&paths.manifest, metadata::MAX_METADATA).map_err(PathFailure::Ambiguous)?
        != previous_bytes
    {
        return Err(PathFailure::Ambiguous(
            "Metadata berubah sebelum edit PATH".into(),
        ));
    }
    let previous_path = meta.path.clone();
    let mut updated = meta.clone();
    updated.path = Some(ownership);
    let stage = Stage::bytes(
        &paths.metadata_workspace(),
        "path-metadata",
        &updated.encode().map_err(PathFailure::Manual)?,
        0o600,
    )
    .map_err(PathFailure::Manual)?;
    let mut manifest = Replacement::new(
        stage,
        &paths.manifest,
        Some(Expected {
            id: previous_id,
            hash: files::hash_bytes(&previous_bytes),
        }),
    )
    .map_err(PathFailure::Manual)?;
    if let Err(error) = edit.apply() {
        edit.rollback().map_err(PathFailure::Ambiguous)?;
        return Err(PathFailure::Manual(error));
    }
    if let Err(error) = manifest.apply() {
        meta.path = previous_path;
        edit.rollback().map_err(PathFailure::Ambiguous)?;
        manifest.rollback().map_err(PathFailure::Ambiguous)?;
        return Err(PathFailure::Manual(error));
    }
    *meta = updated;
    Ok(())
}

pub(crate) fn uninstall() -> ExitCode {
    status((|| uninstall_from(&Paths::from_env()?))())
}

fn uninstall_from(paths: &Paths) -> Result<()> {
    uninstall_with(paths, || Ok(()))
}

fn uninstall_with(paths: &Paths, before_path_commit: impl FnOnce() -> Result<()>) -> Result<()> {
    uninstall_transaction(paths, || Ok(()), || Ok(()), before_path_commit)
}

fn uninstall_transaction(
    paths: &Paths,
    before_binary: impl FnOnce() -> Result<()>,
    before_metadata: impl FnOnce() -> Result<()>,
    before_path_commit: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let _lock = paths.prepare()?;
    let previous = paths.load()?;
    let Some(previous) = previous else {
        binary_owned(paths, None, true)?;
        println!("TTC tidak terdaftar; tidak ada instalasi yang dihapus");
        return Ok(());
    };
    if !previous.meta.active_harnesses.is_empty() {
        return Err("Integrasi harness masih aktif; hapus integrasi terlebih dahulu".into());
    }
    let binary = binary_owned(paths, Some(&previous), true)?;
    let directory = paths.binary.parent().ok_or("Parent binary tidak ada")?;
    let mut image = binary.map(|id| {
        Removal::new(
            &paths.binary,
            &paths.binary_workspace(),
            Expected {
                id,
                hash: previous.meta.sha256.clone(),
            },
        )
    });
    let mut manifest = Removal::new(
        &paths.manifest,
        &paths.metadata_workspace(),
        Expected {
            id: previous.id,
            hash: files::hash_bytes(&previous.bytes),
        },
    );
    let marker = paths.marker("uninstall")?;
    let result = (|| {
        paths.unchanged(Some(&previous))?;
        if binary_owned(paths, Some(&previous), true)? != binary {
            return Err("Binary berubah sebelum uninstall".into());
        }
        if let Some(image) = &mut image {
            image.apply_with(before_binary)?;
        }
        manifest.apply_with(before_metadata)
    })();
    if let Err(error) = result {
        let recovery = (|| {
            if let Some(image) = &mut image {
                image.rollback()?;
            }
            manifest.rollback()
        })();
        if recovery.is_ok() {
            files::remove_owned(&paths.pending, marker)?;
        }
        return Err(format!(
            "{error}; uninstall belum selesai; {}",
            recovery
                .err()
                .unwrap_or_else(|| "rollback selesai, archive dipertahankan".into())
        ));
    }
    let workspace = paths.binary_workspace();
    let others = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .try_fold(false, |found, entry| {
            let entry = entry.map_err(|error| error.to_string())?;
            Ok::<_, String>(found || entry.path() != workspace)
        })?;
    if !others && let Some(ownership) = &previous.meta.path {
        match path::remove(ownership) {
            Ok(Some(mut edit)) => {
                if let Err(error) = edit.apply_with(|| Ok(()), before_path_commit) {
                    if let Err(recovery_error) = edit.rollback() {
                        return Err(format!(
                            "{error}; {recovery_error}; marker transaksi dipertahankan"
                        ));
                    }
                    eprintln!("ttc: blok PATH dipertahankan: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("ttc: blok PATH dipertahankan: {error}"),
        }
    }
    files::remove_owned(&paths.pending, marker)?;
    println!("Instalasi global TTC dihapus");
    Ok(())
}

fn status(result: Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ttc: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture {
        root: PathBuf,
        paths: Paths,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "ttc-m9-unit-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let home = root.join("home spasi ' Unicode-日本");
            fs::create_dir_all(&home).unwrap();
            let data = root.join("data/ttc");
            let paths = Paths {
                binary: home.join(".local/bin/ttc"),
                manifest: data.join("install.toml"),
                pending: data.join("install.pending"),
                home,
                data,
            };
            Self { root, paths }
        }
        fn source(&self, name: &str, bytes: &[u8]) -> (PathBuf, String) {
            let path = self.root.join(name);
            fs::write(&path, bytes).unwrap();
            let hash = files::hash(&path).unwrap();
            (path, hash)
        }
        fn install(&self, source: &Path, hash: &str) {
            install_from(&self.paths, source, hash, || Ok(())).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn save_user_entry(path: &Path, atomic: bool, bytes: &[u8]) -> Result<()> {
        if atomic {
            let temp = path.with_extension("user-save");
            fs::write(&temp, bytes).unwrap();
            fs::rename(temp, path).unwrap();
        } else {
            fs::write(path, bytes).unwrap();
        }
        Ok(())
    }

    fn archive_contains(directory: &Path, bytes: &[u8]) -> bool {
        fs::read_dir(directory)
            .unwrap()
            .any(|entry| fs::read(entry.unwrap().path()).unwrap() == bytes)
    }

    #[test]
    fn commit_races_preserve_binary_metadata_and_pending_marker() {
        for metadata in [false, true] {
            for atomic in [false, true] {
                let fixture = Fixture::new();
                let (first, hash) = fixture.source("first", b"original image");
                fixture.install(&first, &hash);
                let (second, hash) = fixture.source("second", b"new image");
                let latest = b"user save after final ownership validation";
                let result = install_with(
                    &fixture.paths,
                    &second,
                    &hash,
                    || {
                        if metadata {
                            Ok(())
                        } else {
                            save_user_entry(&fixture.paths.binary, atomic, latest)
                        }
                    },
                    || {
                        if metadata {
                            save_user_entry(&fixture.paths.manifest, atomic, latest)
                        } else {
                            Ok(())
                        }
                    },
                );
                assert!(result.is_err());
                assert!(result.unwrap_err().contains("recovery"));
                assert!(fixture.paths.pending.exists());
                let directory = if metadata {
                    fixture.paths.metadata_workspace()
                } else {
                    fixture.paths.binary_workspace()
                };
                assert!(archive_contains(&directory, latest));
                assert!(install_from(&fixture.paths, &second, &hash, || Ok(())).is_err());
                assert!(uninstall_from(&fixture.paths).is_err());
                assert!(archive_contains(&directory, latest));
            }
        }
    }

    #[test]
    fn uninstall_races_preserve_binary_metadata_and_pending_marker() {
        for metadata in [false, true] {
            for atomic in [false, true] {
                let fixture = Fixture::new();
                let (source, hash) = fixture.source("image", b"original image");
                fixture.install(&source, &hash);
                let latest = b"user save during uninstall commit";
                assert!(
                    uninstall_transaction(
                        &fixture.paths,
                        || if metadata {
                            Ok(())
                        } else {
                            save_user_entry(&fixture.paths.binary, atomic, latest)
                        },
                        || if metadata {
                            save_user_entry(&fixture.paths.manifest, atomic, latest)
                        } else {
                            Ok(())
                        },
                        || Ok(())
                    )
                    .is_err()
                );
                assert!(fixture.paths.pending.exists());
                let directory = if metadata {
                    fixture.paths.metadata_workspace()
                } else {
                    fixture.paths.binary_workspace()
                };
                assert!(archive_contains(&directory, latest));
                assert!(install_from(&fixture.paths, &source, &hash, || Ok(())).is_err());
                assert!(uninstall_from(&fixture.paths).is_err());
            }
        }
    }

    #[test]
    fn existing_unowned_workspace_is_not_adopted_or_modified() {
        let fixture = Fixture::new();
        let directory = fixture.paths.binary_workspace();
        fs::create_dir_all(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(directory.join("user-data"), b"unrelated program data").unwrap();
        let (source, hash) = fixture.source("image", b"candidate");
        assert!(install_from(&fixture.paths, &source, &hash, || Ok(())).is_err());
        assert!(!fixture.paths.binary.exists());
        assert!(!directory.join("owner").exists());
        assert_eq!(
            fs::read(directory.join("user-data")).unwrap(),
            b"unrelated program data"
        );
    }

    #[test]
    fn fresh_reinstall_and_global_uninstall_preserve_user_config_and_raw_state() {
        let fixture = Fixture::new();
        let bashrc = fixture.paths.home.join(".bashrc");
        fs::write(&bashrc, "export USER_SETTING=1").unwrap();
        let (source, hash) = fixture.source("candidate", b"candidate image");
        fixture.install(&source, &hash);
        let first_config = fs::read(&bashrc).unwrap();
        fixture.install(&source, &hash);
        assert_eq!(fs::read(&bashrc).unwrap(), first_config);
        let mut updated = first_config;
        updated.extend_from_slice(b"export AFTER_INSTALL=2\n");
        fs::write(&bashrc, updated).unwrap();
        let raw = fixture.paths.home.join(".local/state/ttc/raw");
        fs::create_dir_all(&raw).unwrap();
        fs::write(raw.join("capture"), b"retain").unwrap();
        uninstall_from(&fixture.paths).unwrap();
        assert!(!fixture.paths.binary.exists());
        assert!(!fixture.paths.manifest.exists());
        assert_eq!(
            fs::read(&bashrc).unwrap(),
            b"export USER_SETTING=1\nexport AFTER_INSTALL=2\n"
        );
        assert_eq!(fs::read(raw.join("capture")).unwrap(), b"retain");
        uninstall_from(&fixture.paths).unwrap();
    }

    #[test]
    fn metadata_commit_failure_rolls_back_bytes_permissions_and_manifest() {
        let fixture = Fixture::new();
        let (first, first_hash) = fixture.source("first", b"old image");
        fixture.install(&first, &first_hash);
        fs::set_permissions(&fixture.paths.binary, fs::Permissions::from_mode(0o700)).unwrap();
        let before = fs::read(&fixture.paths.manifest).unwrap();
        let (second, second_hash) = fixture.source("second", b"new image");
        assert!(
            install_from(&fixture.paths, &second, &second_hash, || Err(
                "injected metadata I/O failure".into()
            ))
            .is_err()
        );
        assert_eq!(fs::read(&fixture.paths.binary).unwrap(), b"old image");
        assert_eq!(
            fs::metadata(&fixture.paths.binary)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(fs::read(&fixture.paths.manifest).unwrap(), before);
        assert!(!fixture.paths.pending.exists());
    }

    #[test]
    fn uninstall_path_conflict_keeps_recovery_and_blocks_following_operations() {
        let fixture = Fixture::new();
        let file = fixture.paths.home.join(".bashrc");
        fs::write(&file, b"user config\n").unwrap();
        let (source, hash) = fixture.source("image", b"image");
        fixture.install(&source, &hash);
        let latest = b"user save after final validation\n";
        let error = uninstall_with(&fixture.paths, || {
            let save = fixture.paths.home.join("editor-save");
            fs::write(&save, latest).unwrap();
            fs::rename(save, &file).unwrap();
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("recovery"), "{error}");
        assert!(fixture.paths.pending.exists());
        assert!(fs::read_dir(&fixture.paths.home).unwrap().any(|entry| {
            let path = entry.unwrap().path();
            path.is_file() && fs::read(path).unwrap() == latest
        }));
        assert!(install_from(&fixture.paths, &source, &hash, || Ok(())).is_err());
        assert!(uninstall_from(&fixture.paths).is_err());
        // The removed installation also stays available for manual recovery.
        assert!(
            fs::read_dir(fixture.paths.binary_workspace())
                .unwrap()
                .any(|entry| { fs::read(entry.unwrap().path()).unwrap() == b"image" })
        );
        assert!(
            fs::read_dir(fixture.paths.metadata_workspace())
                .unwrap()
                .any(|entry| {
                    let path = entry.unwrap().path();
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .ends_with("uninstall")
                        && Metadata::parse(
                            &fs::read(path).unwrap(),
                            &fixture.paths.binary,
                            &file,
                            path::BLOCK,
                        )
                        .is_ok()
                })
        );
    }

    #[test]
    fn ambiguous_rollback_preserves_foreign_bytes_and_blocks_recovery() {
        let fixture = Fixture::new();
        let (first, hash) = fixture.source("first", b"old image");
        fixture.install(&first, &hash);
        let (second, hash) = fixture.source("second", b"new image");
        assert!(
            install_from(&fixture.paths, &second, &hash, || {
                fs::write(&fixture.paths.binary, b"user replacement").unwrap();
                Err("injected failure after external replacement".into())
            })
            .is_err()
        );
        assert_eq!(
            fs::read(&fixture.paths.binary).unwrap(),
            b"user replacement"
        );
        assert!(fixture.paths.pending.exists());
        assert!(install_from(&fixture.paths, &second, &hash, || Ok(())).is_err());
        assert!(uninstall_from(&fixture.paths).is_err());
        assert_eq!(
            fs::read(&fixture.paths.binary).unwrap(),
            b"user replacement"
        );
    }

    #[test]
    fn checksum_and_ownership_fail_before_replacing_existing_files() {
        let fixture = Fixture::new();
        let (source, hash) = fixture.source("image", b"image");
        assert!(install_from(&fixture.paths, &source, &"0".repeat(64), || Ok(())).is_err());
        assert!(!fixture.paths.binary.exists());
        fs::write(&fixture.paths.binary, b"unrelated program").unwrap();
        assert!(install_from(&fixture.paths, &source, &hash, || Ok(())).is_err());
        assert!(uninstall_from(&fixture.paths).is_err());
        assert_eq!(
            fs::read(&fixture.paths.binary).unwrap(),
            b"unrelated program"
        );
    }

    #[test]
    fn unknown_metadata_and_registered_harnesses_are_preserved() {
        let fixture = Fixture::new();
        let (source, hash) = fixture.source("image", b"image");
        fixture.install(&source, &hash);
        let mut meta = fixture.paths.load().unwrap().unwrap().meta;
        meta.active_harnesses = vec!["codex".into()];
        fs::write(&fixture.paths.manifest, meta.encode().unwrap()).unwrap();
        fixture.install(&source, &hash);
        assert_eq!(
            fixture.paths.load().unwrap().unwrap().meta.active_harnesses,
            vec!["codex"]
        );
        assert!(uninstall_from(&fixture.paths).is_err());
        let mut table: toml::Table = toml::from_str(
            std::str::from_utf8(&fs::read(&fixture.paths.manifest).unwrap()).unwrap(),
        )
        .unwrap();
        table.insert("schema_version".into(), toml::Value::Integer(2));
        fs::write(&fixture.paths.manifest, toml::to_string(&table).unwrap()).unwrap();
        let before = fs::read(&fixture.paths.manifest).unwrap();
        assert!(install_from(&fixture.paths, &source, &hash, || Ok(())).is_err());
        assert_eq!(fs::read(&fixture.paths.manifest).unwrap(), before);
        assert_eq!(fs::read(&fixture.paths.binary).unwrap(), b"image");
    }

    #[test]
    fn read_only_bashrc_uses_manual_fallback_with_healthy_binary() {
        let fixture = Fixture::new();
        let file = fixture.paths.home.join(".bashrc");
        fs::write(&file, b"user config\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
        let (source, hash) = fixture.source("image", b"image");
        fixture.install(&source, &hash);
        assert!(fixture.paths.load().unwrap().unwrap().meta.path.is_none());
        assert_eq!(fs::read(file).unwrap(), b"user config\n");
        assert_eq!(fs::read(&fixture.paths.binary).unwrap(), b"image");
    }

    #[test]
    fn unrelated_program_and_user_modified_path_block_survive_uninstall() {
        let fixture = Fixture::new();
        let (source, hash) = fixture.source("image", b"image");
        fixture.install(&source, &hash);
        fs::write(
            fixture.paths.binary.parent().unwrap().join("other-tool"),
            b"other",
        )
        .unwrap();
        let before = fs::read(fixture.paths.home.join(".bashrc")).unwrap();
        uninstall_from(&fixture.paths).unwrap();
        assert_eq!(
            fs::read(fixture.paths.home.join(".bashrc")).unwrap(),
            before
        );
        assert!(
            fixture
                .paths
                .binary
                .parent()
                .unwrap()
                .join("other-tool")
                .exists()
        );

        let fixture = Fixture::new();
        let (source, hash) = fixture.source("image", b"image");
        fixture.install(&source, &hash);
        let file = fixture.paths.home.join(".bashrc");
        fs::write(&file, b"# >>> TTC managed PATH >>>\nuser customization\n").unwrap();
        uninstall_from(&fixture.paths).unwrap();
        assert_eq!(
            fs::read(file).unwrap(),
            b"# >>> TTC managed PATH >>>\nuser customization\n"
        );
    }

    #[test]
    fn killed_installer_preserves_pending_transaction_and_blocks_next_operation() {
        const CHILD_ROOT: &str = "TTC_M9_UNIT_KILL_ROOT";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = PathBuf::from(root);
            let home = root.join("home spasi ' Unicode-日本");
            let data = root.join("data/ttc");
            let paths = Paths {
                binary: home.join(".local/bin/ttc"),
                manifest: data.join("install.toml"),
                pending: data.join("install.pending"),
                home,
                data,
            };
            let source = root.join("second");
            let hash = files::hash(&source).unwrap();
            let _ = install_from(&paths, &source, &hash, || {
                nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::Signal::SIGKILL)
                    .unwrap();
                unreachable!("SIGKILL must terminate the process")
            });
            panic!("child must reach the injected interruption");
        }
        use std::os::unix::process::ExitStatusExt;
        let fixture = Fixture::new();
        let (first, hash) = fixture.source("first", b"old image");
        fixture.install(&first, &hash);
        let before = fs::read(&fixture.paths.manifest).unwrap();
        let (second, hash) = fixture.source("second", b"new image");
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "distribution::tests::killed_installer_preserves_pending_transaction_and_blocks_next_operation",
            ])
            .env(CHILD_ROOT, &fixture.root)
            .output()
            .unwrap();
        assert_eq!(child.status.signal(), Some(nix::libc::SIGKILL));
        assert!(fixture.paths.pending.exists());
        assert_eq!(fs::read(&fixture.paths.manifest).unwrap(), before);
        assert_eq!(fs::read(&fixture.paths.binary).unwrap(), b"new image");
        assert!(install_from(&fixture.paths, &second, &hash, || Ok(())).is_err());
        assert!(uninstall_from(&fixture.paths).is_err());
    }

    #[test]
    fn failed_atomic_rename_preserves_destination_and_staging_bytes() {
        let fixture = Fixture::new();
        let destination = fixture.root.join("destination");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("user-data"), b"user-owned content").unwrap();
        let stage = Stage::bytes(&fixture.root, "rename-failure", b"candidate", 0o755).unwrap();
        let stage_path = stage.path.clone();
        let mut replacement = Replacement::new(stage, &destination, None).unwrap();
        assert!(replacement.apply().is_err());
        assert_eq!(
            fs::read(destination.join("user-data")).unwrap(),
            b"user-owned content"
        );
        assert_eq!(fs::read(&stage_path).unwrap(), b"candidate");
    }

    #[test]
    fn interrupted_transaction_marker_refuses_admin_mutations() {
        let fixture = Fixture::new();
        let (source, hash) = fixture.source("image", b"image");
        fixture.install(&source, &hash);
        {
            let _lock = fixture.paths.prepare().unwrap();
            fixture.paths.marker("install").unwrap();
        }
        let before = fs::read(&fixture.paths.manifest).unwrap();
        assert!(install_from(&fixture.paths, &source, &hash, || Ok(())).is_err());
        assert!(uninstall_from(&fixture.paths).is_err());
        assert_eq!(fs::read(&fixture.paths.manifest).unwrap(), before);
        assert_eq!(fs::read(&fixture.paths.binary).unwrap(), b"image");
    }
}
