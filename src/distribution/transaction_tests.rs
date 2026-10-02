use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ttc-entry-race-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("archive")).unwrap();
        Self(root)
    }
    fn target(&self) -> PathBuf {
        self.0.join("target")
    }
    fn expected(&self) -> Expected {
        Expected {
            id: files::identity(&self.target()).unwrap().unwrap(),
            hash: files::hash(&self.target()).unwrap(),
        }
    }
    fn replacement(&self, present: bool) -> Replacement {
        let before = present.then(|| self.expected());
        let stage =
            Stage::bytes(&self.0.join("archive"), "image", b"new candidate", 0o755).unwrap();
        Replacement::new(stage, &self.target(), before).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn save(path: &Path, atomic: bool) -> Result<()> {
    if atomic {
        let temp = path.with_extension("save");
        fs::write(&temp, b"latest user binary").unwrap();
        fs::rename(temp, path).unwrap();
    } else {
        fs::write(path, b"latest user binary").unwrap();
    }
    Ok(())
}

#[test]
fn replacement_keeps_actual_entry_changed_after_last_check() {
    for atomic in [false, true] {
        let fixture = Fixture::new();
        fs::write(fixture.target(), b"original binary").unwrap();
        let mut edit = fixture.replacement(true);
        assert!(edit.apply_with(|| save(&fixture.target(), atomic)).is_err());
        let recovery = edit.recovery.clone().unwrap();
        assert_eq!(fs::read(&recovery).unwrap(), b"latest user binary");
        assert!(edit.rollback().is_err());
        drop(edit);
        assert_eq!(fs::read(recovery).unwrap(), b"latest user binary");
    }
}

#[test]
fn rollback_keeps_actual_entry_changed_after_last_check() {
    for present in [false, true] {
        for atomic in [false, true] {
            let fixture = Fixture::new();
            if present {
                fs::write(fixture.target(), b"original binary").unwrap();
            }
            let mut edit = fixture.replacement(present);
            edit.apply().unwrap();
            assert!(
                edit.rollback_with(|| save(&fixture.target(), atomic))
                    .is_err()
            );
            let recovery = edit.recovery.clone().unwrap();
            assert_eq!(fs::read(&recovery).unwrap(), b"latest user binary");
            drop(edit);
            assert_eq!(fs::read(recovery).unwrap(), b"latest user binary");
        }
    }
}

#[test]
fn removal_keeps_actual_entry_changed_after_last_check() {
    for atomic in [false, true] {
        let fixture = Fixture::new();
        fs::write(fixture.target(), b"original binary").unwrap();
        let mut removal = Removal::new(
            &fixture.target(),
            &fixture.0.join("archive"),
            fixture.expected(),
        );
        assert!(
            removal
                .apply_with(|| save(&fixture.target(), atomic))
                .is_err()
        );
        let recovery = removal.recovery.clone().unwrap();
        assert!(removal.rollback().is_err());
        drop(removal);
        assert_eq!(fs::read(recovery).unwrap(), b"latest user binary");
    }
}

#[test]
fn successful_replacement_keeps_original_inode_for_late_fd_writes() {
    use std::io::Write;
    let fixture = Fixture::new();
    fs::write(fixture.target(), b"original binary").unwrap();
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .open(fixture.target())
        .unwrap();
    let mut edit = fixture.replacement(true);
    edit.apply().unwrap();
    let archive = edit.stage.path.clone();
    drop(edit);
    writer.write_all(b"late user write").unwrap();
    writer.sync_all().unwrap();
    assert_eq!(fs::read(archive).unwrap(), b"late user write");
    assert_eq!(fs::read(fixture.target()).unwrap(), b"new candidate");
}

#[test]
fn rollback_detects_a_late_write_to_the_original_fd_after_source_validation() {
    use std::io::Write;
    let fixture = Fixture::new();
    fs::write(fixture.target(), b"original binary").unwrap();
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .open(fixture.target())
        .unwrap();
    let mut edit = fixture.replacement(true);
    edit.apply().unwrap();
    assert!(
        edit.rollback_with(|| {
            writer.write_all(b"late user write").unwrap();
            writer.sync_all().unwrap();
            Ok(())
        })
        .is_err()
    );
    assert!(edit.ambiguous);
    assert_eq!(fs::read(fixture.target()).unwrap(), b"late user write");
    assert_eq!(
        fs::read(edit.recovery.as_ref().unwrap()).unwrap(),
        b"new candidate"
    );
}

#[test]
fn removal_rollback_refuses_concurrently_recreated_target() {
    let fixture = Fixture::new();
    fs::write(fixture.target(), b"original binary").unwrap();
    let mut removal = Removal::new(
        &fixture.target(),
        &fixture.0.join("archive"),
        fixture.expected(),
    );
    removal.apply_with(|| Ok(())).unwrap();
    let archive = removal.recovery.clone().unwrap();
    fs::write(fixture.target(), b"user creation").unwrap();
    assert!(removal.rollback().is_err());
    assert_eq!(fs::read(fixture.target()).unwrap(), b"user creation");
    assert_eq!(fs::read(archive).unwrap(), b"original binary");
}

#[test]
fn raced_symlink_is_preserved_without_following_its_target() {
    let fixture = Fixture::new();
    fs::write(fixture.target(), b"original binary").unwrap();
    let foreign = fixture.0.join("foreign");
    fs::write(&foreign, b"unrelated").unwrap();
    let mut edit = fixture.replacement(true);
    assert!(
        edit.apply_with(|| {
            fs::remove_file(fixture.target()).unwrap();
            std::os::unix::fs::symlink(&foreign, fixture.target()).unwrap();
            Ok(())
        })
        .is_err()
    );
    let archive = edit.recovery.clone().unwrap();
    drop(edit);
    assert_eq!(fs::read_link(archive).unwrap(), foreign);
    assert_eq!(fs::read(foreign).unwrap(), b"unrelated");
}

#[test]
fn failed_exchange_after_validation_preserves_candidate_and_user_moved_original() {
    let fixture = Fixture::new();
    fs::write(fixture.target(), b"original binary").unwrap();
    let mut edit = fixture.replacement(true);
    let saved = fixture.0.join("user-moved-original");
    assert!(
        edit.apply_with(|| {
            fs::rename(fixture.target(), &saved).unwrap();
            Ok(())
        })
        .is_err()
    );
    assert!(!edit.applied);
    assert!(edit.rollback().is_ok());
    assert!(!fixture.target().exists());
    assert_eq!(fs::read(saved).unwrap(), b"original binary");
    assert_eq!(fs::read(&edit.stage.path).unwrap(), b"new candidate");
}
