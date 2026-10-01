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
    }))
}

impl Edit {
    pub fn apply(&mut self) -> Result<()> {
        if files::identity(&self.file)? != self.before_id
            || (self.before_id.is_some() && files::read(&self.file, LIMIT)? != self.before)
        {
            return Err("Config Bash berubah sebelum edit".into());
        }
        let parent = self.file.parent().ok_or("Parent config tidak ada")?;
        let mut backup = Stage::bytes(parent, "bashrc-backup", &self.before, 0o600)?;
        backup.keep();
        let mut stage = Stage::bytes(parent, "bashrc", &self.after, self.mode)?;
        self.applied = Some(stage.id());
        if self.before_id.is_some() {
            stage.replace(&self.file)
        } else {
            stage.publish(&self.file)
        }
    }

    pub fn rollback(&mut self) -> Result<()> {
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
        if self.before_id.is_none() {
            files::remove_owned(&self.file, id)
        } else {
            let parent = self.file.parent().ok_or("Parent config tidak ada")?;
            Stage::bytes(parent, "bashrc-rollback", &self.before, self.mode)?.replace(&self.file)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
