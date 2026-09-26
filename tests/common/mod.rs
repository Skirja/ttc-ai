#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub struct TestDir(PathBuf);

impl TestDir {
    pub fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ttc-m2-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

pub fn ttc() -> &'static str {
    env!("CARGO_BIN_EXE_ttc")
}

thread_local! {
    static RUNTIME_DIR: TestDir = TestDir::new();
}

pub fn ttc_command() -> Command {
    RUNTIME_DIR.with(|dir| {
        let mut command = Command::new(ttc());
        command.env("HOME", dir.path());
        command.env("XDG_CONFIG_HOME", dir.path().join("config"));
        command.env("XDG_DATA_HOME", dir.path().join("data"));
        command.env("XDG_STATE_HOME", dir.path().join("state"));
        command.env("TMPDIR", dir.path().join("tmp"));
        command.env("TTC_INTERNAL_TEST_TMP_ROOT", dir.path().join("tmp"));
        command
    })
}
