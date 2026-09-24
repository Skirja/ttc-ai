#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use core::config::Config;
use core::raw_store::{self, StorePaths, Stream};
use core::streaming::{self, CompactKind, Filter};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

struct Synthetic;
impl Filter for Synthetic {
    fn can_compact(&self) -> bool {
        true
    }
    fn decide(&mut self, _stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        if line == b"PASS\n" {
            Ok(Some(CompactKind::Passing))
        } else {
            Ok(None)
        }
    }
}

fn roots(dir: &TestDir) -> StorePaths {
    StorePaths([
        dir.path().join("state/ttc/runs"),
        dir.path()
            .join(format!("temp/ttc-{}/runs", nix::unistd::getuid().as_raw())),
    ])
}

#[test]
fn denied_xdg_uses_user_only_fallback() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("state")).unwrap();
    fs::write(dir.path().join("state/ttc"), b"blocked").unwrap();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender.send((Stream::Stdout, b"PASS\n".to_vec())).unwrap();
    drop(sender);
    let mut out = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(roots(&dir)),
        &mut Synthetic,
        &mut out,
        &mut Vec::new(),
    );
    assert!(out.is_empty());
    assert_eq!(report.passing, 1);
    let id = report.raw_id.unwrap();
    let fallback = roots(&dir).0[1].clone();
    assert!(fallback.join(format!("{id}.raw")).exists());
    assert_eq!(
        fs::metadata(&fallback).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(fallback.join(format!("{id}.raw")))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let replay = ttc_command()
        .args(["raw", &id, "--stdout"])
        .env("HOME", dir.path())
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .env("TMPDIR", dir.path().join("temp"))
        .output()
        .unwrap();
    assert!(replay.status.success(), "{replay:?}");
    assert_eq!(replay.stdout, b"PASS\n");
}

#[test]
fn both_storage_failures_emit_raw_and_never_rerun_child() {
    let dir = TestDir::new();
    let paths = roots(&dir);
    for path in &paths.0 {
        fs::create_dir_all(path.parent().unwrap().parent().unwrap()).unwrap();
        fs::write(path.parent().unwrap(), b"blocked").unwrap();
    }
    let count = dir.path().join("count");
    let mut child = Command::new("/bin/sh")
        .args(["-c", "printf x >> \"$COUNT\"; printf 'PASS\\nPASS\\n'"])
        .env("COUNT", &count)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let (sender, receiver) = mpsc::sync_channel(8);
    let stdout = child.stdout.take().unwrap();
    let out_reader = thread::spawn(move || streaming::read_stream(stdout, Stream::Stdout, sender));
    let mut out = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(roots(&dir)),
        &mut Synthetic,
        &mut out,
        &mut Vec::new(),
    );
    out_reader.join().unwrap().unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(out, b"PASS\nPASS\n");
    assert_eq!(report.passing, 0);
    assert!(report.raw_id.is_none());
    assert_eq!(fs::read(count).unwrap(), b"x");
}

#[test]
fn cleanup_removes_only_expired_captures() {
    let dir = TestDir::new();
    let roots = roots(&dir);
    let mut capture = raw_store::Capture::new(Config::default(), roots.clone());
    capture.record(Stream::Stdout, b"old").unwrap();
    let id = capture.enable(Config::default()).unwrap().to_owned();
    capture.finish().unwrap();
    let old = roots.0[0].join(format!("{id}.raw"));
    let mut current = raw_store::Capture::new(Config::default(), roots.clone());
    current.record(Stream::Stdout, b"new").unwrap();
    let current_id = current.enable(Config::default()).unwrap().to_owned();
    current.finish().unwrap();
    let current_path = roots.0[0].join(format!("{current_id}.raw"));
    let old_time = SystemTime::now() - Duration::from_secs(25 * 3600);
    fs::File::open(&old)
        .unwrap()
        .set_modified(old_time)
        .unwrap();
    raw_store::cleanup_paths(Config::default(), &roots);
    assert!(!old.exists());
    assert!(current_path.exists());
}
