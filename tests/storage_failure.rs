#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::config::Config;
use core::raw_store::{self, Selection, StorePaths, Stream};
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
    let state = dir.path().join("state");
    fs::create_dir_all(&state).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o000)).unwrap();
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
    let (mut replay_out, mut replay_err) = (Vec::new(), Vec::new());
    let dropped = raw_store::replay_at_to(
        &roots(&dir),
        &id,
        Selection::Stdout,
        None,
        &mut replay_out,
        &mut replay_err,
    )
    .unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(dropped, 0);
    assert_eq!(replay_out, b"PASS\n");
    assert!(replay_err.is_empty());
}

#[test]
fn fallback_root_uses_stable_system_temp_directory() {
    let expected = format!("/tmp/ttc-{}/runs", nix::unistd::getuid().as_raw());
    assert_eq!(
        StorePaths::from_env().unwrap().0[1].to_string_lossy(),
        expected
    );
}

#[test]
fn failed_append_keeps_last_committed_capture() {
    let dir = TestDir::new();
    let config = Config {
        max_raw_mb: 1,
        retention_hours: 24,
    };
    let paths = roots(&dir);
    let mut capture = raw_store::Capture::new(config, paths.clone());
    capture.record(Stream::Stdout, b"first\n").unwrap();
    let id = capture.enable(config).unwrap().to_owned();
    capture
        .record(Stream::Stdout, &vec![b'a'; 2 * 1024 * 1024])
        .unwrap();
    let mut before = Vec::new();
    raw_store::replay_at_to(
        &paths,
        &id,
        Selection::Stdout,
        None,
        &mut before,
        &mut Vec::new(),
    )
    .unwrap();
    capture.inject_append_failure();
    assert!(
        capture
            .record(Stream::Stdout, &vec![b'z'; 32 * 1024])
            .is_err()
    );
    assert_eq!(capture.finish().unwrap(), Some(id.as_str()));
    let mut after = Vec::new();
    raw_store::replay_at_to(
        &paths,
        &id,
        Selection::Stdout,
        None,
        &mut after,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(after, before);
}

#[test]
fn rename_failure_keeps_partial_file_replayable() {
    let dir = TestDir::new();
    let config = Config {
        max_raw_mb: 1,
        retention_hours: 24,
    };
    let paths = roots(&dir);
    let mut capture = raw_store::Capture::new(config, paths.clone());
    capture
        .record(Stream::Stderr, b"prior diagnostic\n")
        .unwrap();
    let id = capture.enable(config).unwrap().to_owned();
    capture.inject_rename_failure();
    assert_eq!(capture.finish().unwrap(), Some(id.as_str()));
    assert!(paths.0[0].join(format!("{id}.part")).exists());
    let mut out = Vec::new();
    raw_store::replay_at_to(
        &paths,
        &id,
        Selection::Stderr,
        None,
        &mut out,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(out, b"prior diagnostic\n");
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
