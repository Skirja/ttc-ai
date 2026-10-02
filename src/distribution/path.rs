//! Bounded Bash configuration edits with exact ownership matching.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::files::{self, Identity, Result, Stage};
use super::metadata::{PathOwnership, text_path};

pub(super) const BLOCK: &str = "# >>> TTC managed PATH >>>\ncase \":${PATH-}:\" in\n  *\":$HOME/.local/bin:\"*) ;;\n  *) export PATH=\"$HOME/.local/bin${PATH:+:$PATH}\" ;;\nesac\n# <<< TTC managed PATH <<<\n";
const LIMIT: usize = 1024 * 1024;

pub(super) fn quote(value: &Path) -> String {
    format!("'{}'", value.to_string_lossy().replace('\'', "'\"'\"'"))
}

pub(super) struct Edit {
    file: PathBuf,
    before: Vec<u8>,
    before_id: Option<Identity>,
    after: Vec<u8>,
    mode: u32,
    applied: Option<Identity>,
    recovery: Option<PathBuf>,
    ambiguous: bool,
}

fn contents(file: &Path) -> Result<(Vec<u8>, Option<Identity>, u32)> {
    let id = files::identity(file)?;
    let Some(id) = id else {
        return Ok((Vec::new(), None, 0o644));
    };
    let mode = fs::metadata(file)
        .map_err(|error| error.to_string())?
        .permissions()
        .mode()
        & 0o777;
    if mode & 0o200 == 0 {
        return Err("Config Bash read-only".into());
    }
    let bytes = files::read(file, LIMIT)?;
    std::str::from_utf8(&bytes).map_err(|_| "Config Bash bukan UTF-8")?;
    Ok((bytes, Some(id), mode))
}

pub(super) fn append(
    file: &Path,
    binary_dir: &Path,
    previous: Option<&PathOwnership>,
) -> Result<Option<(Edit, PathOwnership)>> {
    if std::env::var_os("PATH")
        .is_some_and(|value| std::env::split_paths(&value).any(|path| path == binary_dir))
    {
        return Ok(None);
    }
    let (before, before_id, mode) = contents(file)?;
    let text = std::str::from_utf8(&before).map_err(|_| "Config Bash bukan UTF-8")?;
    if let Some(previous) = previous {
        if text.matches(&previous.block).count() == 1 {
            return Ok(None);
        }
        return Err("Blok PATH milik TTC telah diubah; config dipertahankan".into());
    }
    if text.contains("TTC managed PATH") {
        return Err("Blok PATH existing tidak terdaftar atau ambigu".into());
    }
    let block = if before.is_empty() || before.ends_with(b"\n") {
        BLOCK.to_owned()
    } else {
        format!("\n{BLOCK}")
    };
    let mut after = before.clone();
    after.extend_from_slice(block.as_bytes());
    Ok(Some((
        Edit {
            file: file.to_owned(),
            before,
            before_id,
            after,
            mode,
            applied: None,
            recovery: None,
            ambiguous: false,
        },
        PathOwnership {
            file: text_path(file)?,
            block,
        },
    )))
}

pub(super) fn remove(ownership: &PathOwnership) -> Result<Option<Edit>> {
    let file = PathBuf::from(&ownership.file);
    let (before, before_id, mode) = contents(&file)?;
    let text = std::str::from_utf8(&before).map_err(|_| "Config Bash bukan UTF-8")?;
    if text.matches(&ownership.block).count() != 1 {
        return Ok(None);
    }
    let start = text.find(&ownership.block).ok_or("Blok PATH tidak ada")?;
    let end = start + ownership.block.len();
    let mut after = text.as_bytes()[..start].to_vec();
    if ownership.block.starts_with('\n') && end < text.len() && !after.ends_with(b"\n") {
        after.push(b'\n');
    }
    after.extend_from_slice(&text.as_bytes()[end..]);
    Ok(Some(Edit {
        file,
        before,
        before_id,
        after,
        mode,
        applied: None,
        recovery: None,
        ambiguous: false,
    }))
}

impl Edit {
    pub fn apply(&mut self) -> Result<()> {
        self.apply_with(|| Ok(()), || Ok(()))
    }

    pub(super) fn apply_with(
        &mut self,
        after_staging: impl FnOnce() -> Result<()>,
        before_commit: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        if files::identity(&self.file)? != self.before_id
            || (self.before_id.is_some() && files::read(&self.file, LIMIT)? != self.before)
        {
            return Err("Config Bash berubah sebelum edit".into());
        }
        let parent = self.file.parent().ok_or("Parent config tidak ada")?;
        let mut backup = Stage::bytes(parent, "bashrc-backup", &self.before, 0o600)?;
        backup.keep();
        let mut stage = Stage::bytes(parent, "bashrc", &self.after, self.mode)?;
        after_staging()?;
        if files::identity(&self.file)? != self.before_id
            || (self.before_id.is_some() && files::read(&self.file, LIMIT)? != self.before)
        {
            return Err("Config Bash berubah selama staging; perubahan dipertahankan".into());
        }
        before_commit()?;
        if self.before_id.is_some() {
            stage.exchange(&self.file)?;
            self.applied = Some(stage.id());
            self.recovery = Some(stage.path.clone());
            self.ambiguous = true;
            files::sync_directory(parent)?;
            // Check the entry actually displaced by the atomic operation,
            // not the destination observed before an editor could save it.
            if files::identity(&stage.path)? != self.before_id
                || files::read(&stage.path, LIMIT)? != self.before
            {
                return Err(self.recovery_error());
            }
            self.ambiguous = false;
            Ok(())
        } else {
            self.applied = Some(stage.id());
            stage.publish(&self.file)
        }
    }

    fn recovery_error(&self) -> String {
        format!(
            "Config berubah atau commit ambigu; config saat ini dan file recovery {} dipertahankan",
            self.recovery.as_ref().unwrap_or(&self.file).display()
        )
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.rollback_with(|| Ok(()))
    }

    fn rollback_with(&mut self, before_commit: impl FnOnce() -> Result<()>) -> Result<()> {
        if self.ambiguous {
            return Err(self.recovery_error());
        }
        let Some(id) = self.applied else {
            return Ok(());
        };
        if files::identity(&self.file)? == self.before_id {
            return Ok(());
        }
        if files::identity(&self.file)? != Some(id) || files::read(&self.file, LIMIT)? != self.after
        {
            return Err("Config berubah; rollback otomatis ditolak".into());
        }
        let parent = self.file.parent().ok_or("Parent config tidak ada")?;
        before_commit()?;
        if let Some(recovery) = &self.recovery {
            if files::identity(recovery)? != self.before_id
                || files::read(recovery, LIMIT)? != self.before
            {
                return Err(self.recovery_error());
            }
            files::exchange(recovery, &self.file)?;
            self.ambiguous = true;
        } else {
            self.recovery = Some(files::capture(&self.file)?);
            self.ambiguous = true;
        }
        files::sync_directory(parent)?;
        let recovery = self.recovery.as_ref().ok_or("Lokasi recovery tidak ada")?;
        if files::identity(recovery)? != Some(id) || files::read(recovery, LIMIT)? != self.after {
            return Err(self.recovery_error());
        }
        self.applied = None;
        self.ambiguous = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "ttc-path-exchange-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn save(file: &Path, bytes: &[u8], atomic: bool) -> Result<()> {
        if atomic {
            let temporary = file.with_extension("editor-save");
            fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
            fs::rename(temporary, file).map_err(|error| error.to_string())
        } else {
            fs::write(file, bytes).map_err(|error| error.to_string())
        }
    }

    #[test]
    fn apply_preserves_actual_save_after_final_validation() {
        for atomic in [false, true] {
            let root = TestDirectory::new();
            let file = root.0.join(".bashrc");
            fs::write(&file, b"old user setting\n").unwrap();
            let latest = b"save after final validation\n";
            let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
            assert!(
                edit.apply_with(|| Ok(()), || save(&file, latest, atomic))
                    .is_err()
            );
            let recovery = edit.recovery.clone().unwrap();
            assert_eq!(fs::read(&recovery).unwrap(), latest);
            assert_eq!(fs::read(&file).unwrap(), edit.after);
            assert!(
                edit.rollback().is_err(),
                "conflict requires manual recovery"
            );
            drop(edit);
            assert_eq!(
                fs::read(recovery).unwrap(),
                latest,
                "Drop must preserve the displaced save"
            );
        }
    }

    #[test]
    fn apply_preserves_symlink_saved_after_final_validation() {
        let root = TestDirectory::new();
        let file = root.0.join(".bashrc");
        let target = root.0.join("user-target");
        fs::write(&file, b"old config\n").unwrap();
        fs::write(&target, b"unrelated target\n").unwrap();
        let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
        assert!(
            edit.apply_with(
                || Ok(()),
                || {
                    fs::remove_file(&file).unwrap();
                    std::os::unix::fs::symlink(&target, &file).unwrap();
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(edit.rollback().is_err());
        let recovery = edit.recovery.clone().unwrap();
        drop(edit);
        assert_eq!(fs::read_link(recovery).unwrap(), target);
        assert_eq!(fs::read(target).unwrap(), b"unrelated target\n");
    }

    #[test]
    fn fresh_apply_does_not_overwrite_a_concurrent_creation() {
        let root = TestDirectory::new();
        let file = root.0.join(".bashrc");
        let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
        assert!(
            edit.apply_with(|| Ok(()), || save(&file, b"new user config\n", true))
                .is_err()
        );
        assert_eq!(fs::read(file).unwrap(), b"new user config\n");
    }

    #[test]
    fn rollback_preserves_actual_save_after_final_validation() {
        for initially_present in [false, true] {
            for atomic in [false, true] {
                let root = TestDirectory::new();
                let file = root.0.join(".bashrc");
                if initially_present {
                    fs::write(&file, b"original config\n").unwrap();
                }
                let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
                edit.apply().unwrap();
                let latest = b"user save during rollback\n";
                assert!(edit.rollback_with(|| save(&file, latest, atomic)).is_err());
                let recovery = edit.recovery.clone().unwrap();
                assert_eq!(fs::read(&recovery).unwrap(), latest);
                drop(edit);
                assert_eq!(fs::read(recovery).unwrap(), latest);
            }
        }
    }

    #[test]
    fn successful_edit_retains_actual_original_and_rollback_is_atomic() {
        let root = TestDirectory::new();
        let file = root.0.join(".bashrc");
        fs::write(&file, b"original config\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
        let original_id = files::identity(&file).unwrap();
        let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
        edit.apply().unwrap();
        assert_eq!(
            files::identity(edit.recovery.as_ref().unwrap()).unwrap(),
            original_id
        );
        let after = edit.after.clone();
        edit.rollback().unwrap();
        assert_eq!(files::identity(&file).unwrap(), original_id);
        assert_eq!(fs::read(&file).unwrap(), b"original config\n");
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::read(edit.recovery.as_ref().unwrap()).unwrap(), after);
    }

    #[test]
    fn successful_fresh_rollback_retains_removed_managed_config() {
        let root = TestDirectory::new();
        let file = root.0.join(".bashrc");
        let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
        edit.apply().unwrap();
        edit.rollback().unwrap();
        assert!(!file.exists());
        assert_eq!(
            fs::read(edit.recovery.as_ref().unwrap()).unwrap(),
            edit.after
        );
    }

    #[test]
    fn rollback_rejects_edits_to_original_displaced_inode() {
        let root = TestDirectory::new();
        let file = root.0.join(".bashrc");
        fs::write(&file, b"original config\n").unwrap();
        let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
        edit.apply().unwrap();
        let recovery = edit.recovery.clone().unwrap();
        fs::write(&recovery, b"editor wrote its already open inode\n").unwrap();
        assert!(edit.rollback().is_err());
        drop(edit);
        assert_eq!(
            fs::read(recovery).unwrap(),
            b"editor wrote its already open inode\n"
        );
    }

    #[test]
    fn rollback_preserves_in_place_user_changes() {
        let root = std::env::temp_dir().join(format!("ttc-path-rollback-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let file = root.join(".bashrc");
        fs::write(&file, b"user setting\n").unwrap();
        let (mut edit, _) = append(&file, &root.join("bin"), None).unwrap().unwrap();
        edit.apply().unwrap();
        fs::write(&file, b"user edited in place\n").unwrap();
        assert!(edit.rollback().is_err());
        assert_eq!(fs::read(&file).unwrap(), b"user edited in place\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn apply_preserves_user_changes_during_backup_and_staging() {
        struct TestDirectory(PathBuf);
        impl Drop for TestDirectory {
            fn drop(&mut self) {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }

        for atomic_save in [false, true] {
            let root = TestDirectory(std::env::temp_dir().join(format!(
                "ttc-path-staging-{}-{atomic_save}",
                std::process::id()
            )));
            fs::create_dir(&root.0).unwrap();
            let file = root.0.join(".bashrc");
            let initial = b"export USER_SETTING=old\n";
            let latest = b"export USER_SETTING=new\n";
            fs::write(&file, initial).unwrap();
            let (mut edit, _) = append(&file, &root.0.join("bin"), None).unwrap().unwrap();
            let result = edit.apply_with(
                || {
                    if atomic_save {
                        let user_stage = root.0.join(".bashrc.user-save");
                        fs::write(&user_stage, latest).map_err(|error| error.to_string())?;
                        fs::rename(user_stage, &file).map_err(|error| error.to_string())?;
                    } else {
                        fs::write(&file, latest).map_err(|error| error.to_string())?;
                    }
                    Ok(())
                },
                || Ok(()),
            );
            assert!(result.is_err(), "atomic save={atomic_save}");
            assert_eq!(
                fs::read(&file).unwrap(),
                latest,
                "atomic save={atomic_save}"
            );
            assert!(edit.rollback().is_ok(), "no TTC edit was committed");
        }
    }
}
