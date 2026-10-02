//! Preserve the actual entry at the mutation boundary, including late writes.

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use super::files::{self, Identity, Result, Stage};

#[derive(Clone)]
pub(super) struct Expected {
    pub id: Identity,
    pub hash: String,
}

impl Expected {
    pub fn matches(&self, path: &Path) -> Result<bool> {
        Ok(files::identity(path)? == Some(self.id)
            && files::hash(path)? == self.hash
            && files::identity(path)? == Some(self.id))
    }
}

pub(super) struct Replacement {
    stage: Stage,
    target: PathBuf,
    before: Option<Expected>,
    after: Expected,
    applied: bool,
    ambiguous: bool,
    recovery: Option<PathBuf>,
}

impl Replacement {
    pub fn new(stage: Stage, target: &Path, before: Option<Expected>) -> Result<Self> {
        let after = Expected {
            id: stage.id(),
            hash: files::hash(&stage.path)?,
        };
        Ok(Self {
            stage,
            target: target.into(),
            before,
            after,
            applied: false,
            ambiguous: false,
            recovery: None,
        })
    }

    fn error(&self) -> String {
        format!(
            "Transaksi {} ambigu; target dan recovery {} dipertahankan",
            self.target.display(),
            self.recovery.as_ref().unwrap_or(&self.stage.path).display()
        )
    }

    pub fn apply(&mut self) -> Result<()> {
        self.apply_with(|| Ok(()))
    }

    pub fn apply_with(&mut self, before_commit: impl FnOnce() -> Result<()>) -> Result<()> {
        let unchanged = match &self.before {
            Some(before) => before.matches(&self.target)?,
            None => files::identity(&self.target)?.is_none(),
        };
        if !unchanged {
            return Err("Target berubah sebelum replacement".into());
        }
        if !self.after.matches(&self.stage.path)? {
            return Err("Staging berubah".into());
        }
        before_commit()?;
        if let Some(before) = &self.before {
            self.stage.exchange(&self.target)?;
            self.applied = true;
            self.ambiguous = true;
            self.recovery = Some(self.stage.path.clone());
            files::sync_pair(&self.stage.path, &self.target)?;
            if !before.matches(&self.stage.path)? {
                return Err(self.error());
            }
        } else {
            // publish can fail during sync after creating the hard link.
            // Keep the inode so a writer using the published path/FD is safe.
            self.stage.keep();
            self.applied = true;
            self.ambiguous = true;
            self.recovery = Some(self.stage.path.clone());
            self.stage.publish(&self.target)?;
        }
        if !self.after.matches(&self.target)? {
            return Err(self.error());
        }
        self.ambiguous = false;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        self.rollback_with(|| Ok(()))
    }

    fn rollback_with(&mut self, before_commit: impl FnOnce() -> Result<()>) -> Result<()> {
        if self.ambiguous {
            return Err(self.error());
        }
        if !self.applied {
            return Ok(());
        }
        if files::identity(&self.target)?.is_none() && self.before.is_none() {
            return Ok(());
        }
        if !self.after.matches(&self.target)? {
            return Err(self.error());
        }
        if let Some(before) = &self.before
            && !before.matches(&self.stage.path)?
        {
            return Err(self.error());
        }
        before_commit()?;
        if self.before.is_some() {
            files::exchange(&self.stage.path, &self.target)?;
            self.recovery = Some(self.stage.path.clone());
        } else {
            self.recovery = Some(files::capture_in(
                &self.target,
                self.stage.path.parent().ok_or("Parent staging tidak ada")?,
                "rollback",
            )?);
        }
        self.ambiguous = true;
        let recovery = self.recovery.as_ref().ok_or("Recovery tidak ada")?;
        files::sync_pair(recovery, &self.target)?;
        if !self.after.matches(recovery)? {
            return Err(self.error());
        }
        let restored = match &self.before {
            Some(before) => before.matches(&self.target)?,
            None => files::identity(&self.target)?.is_none(),
        };
        if !restored {
            return Err(self.error());
        }
        self.applied = false;
        self.ambiguous = false;
        Ok(())
    }
}

pub(super) struct Removal {
    target: PathBuf,
    directory: PathBuf,
    expected: Expected,
    recovery: Option<PathBuf>,
    ambiguous: bool,
}

impl Removal {
    pub fn new(target: &Path, directory: &Path, expected: Expected) -> Self {
        Self {
            target: target.into(),
            directory: directory.into(),
            expected,
            recovery: None,
            ambiguous: false,
        }
    }

    fn error(&self) -> String {
        format!(
            "Uninstall {} ambigu; recovery {} dipertahankan",
            self.target.display(),
            self.recovery.as_ref().unwrap_or(&self.target).display()
        )
    }

    pub fn apply_with(&mut self, before_commit: impl FnOnce() -> Result<()>) -> Result<()> {
        if !self.expected.matches(&self.target)? {
            return Err("Target berubah sebelum uninstall".into());
        }
        before_commit()?;
        let recovery = files::capture_in(&self.target, &self.directory, "uninstall")?;
        self.recovery = Some(recovery.clone());
        self.ambiguous = true;
        files::sync_pair(&recovery, &self.target)?;
        if !self.expected.matches(&recovery)? {
            return Err(self.error());
        }
        if files::identity(&self.target)?.is_some() {
            return Err(self.error());
        }
        self.ambiguous = false;
        Ok(())
    }

    pub fn rollback(&mut self) -> Result<()> {
        if self.ambiguous {
            return Err(self.error());
        }
        let Some(recovery) = &self.recovery else {
            return Ok(());
        };
        if !self.expected.matches(recovery)? {
            return Err(self.error());
        }
        // A racing creation at target makes this fail without overwriting it.
        fs::hard_link(recovery, &self.target)
            .map_err(|error| format!("{}: {error}", self.error()))?;
        self.ambiguous = true;
        files::sync_pair(recovery, &self.target)?;
        if !self.expected.matches(&self.target)? {
            return Err(self.error());
        }
        self.recovery = None;
        self.ambiguous = false;
        Ok(())
    }
}
