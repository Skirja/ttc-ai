#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use core::config::Config;
use core::raw_store::{StorePaths, Stream};
use core::streaming::{self, CompactKind, Filter, Retain};
use std::sync::mpsc;

struct Synthetic;
impl Filter for Synthetic {
    fn can_compact(&self) -> bool {
        true
    }
    fn decide(&mut self, _stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        if line == b"PASS\n" {
            Ok(Some(CompactKind::Passing))
        } else if line == b"PROGRESS\n" {
            Ok(Some(CompactKind::Progress))
        } else if line == b"BROKEN\n" {
            Err(())
        } else {
            Ok(None)
        }
    }
}

fn paths(dir: &TestDir) -> StorePaths {
    StorePaths([
        dir.path().join("state/ttc/runs"),
        dir.path().join("temp/ttc/runs"),
    ])
}

#[test]
fn per_stream_framing_compacts_only_complete_safe_records() {
    let dir = TestDir::new();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender.send((Stream::Stdout, b"PA".to_vec())).unwrap();
    sender
        .send((Stream::Stderr, b"warning\n".to_vec()))
        .unwrap();
    sender
        .send((Stream::Stdout, b"SS\nother\n".to_vec()))
        .unwrap();
    sender
        .send((Stream::Stdout, b"PROGRESS\n".to_vec()))
        .unwrap();
    drop(sender);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths(&dir)),
        &mut Synthetic,
        &mut out,
        &mut err,
    );
    assert_eq!(out, b"other\n");
    assert_eq!(err, b"warning\n");
    assert_eq!(report.passing, 1);
    assert_eq!(report.progress, 1);
    assert!(report.raw_id.is_some());
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    assert!(
        String::from_utf8(metadata)
            .unwrap()
            .contains("1 passing records and 1 progress records compacted")
    );
}

#[test]
fn invalid_utf8_and_long_line_fail_open_without_losing_bytes() {
    for bad in [
        vec![0xff, b'\n'],
        vec![b'x'; 1024 * 1024 + 1],
        b"BROKEN\n".to_vec(),
    ] {
        let dir = TestDir::new();
        let (sender, receiver) = mpsc::sync_channel(8);
        sender.send((Stream::Stdout, bad.clone())).unwrap();
        sender.send((Stream::Stdout, b"PASS\n".to_vec())).unwrap();
        drop(sender);
        let mut out = Vec::new();
        let report = streaming::process_to(
            receiver,
            Config::default(),
            Some(paths(&dir)),
            &mut Synthetic,
            &mut out,
            &mut Vec::new(),
        );
        assert_eq!(out, [bad, b"PASS\n".to_vec()].concat());
        assert_eq!(report.passing, 0);
        assert!(report.raw_id.is_none());
        assert!(!paths(&dir).0[0].exists());
    }
}

#[test]
fn retain_mode_streams_unknown_bytes_without_capture() {
    let dir = TestDir::new();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender.send((Stream::Stderr, b"\0\xff".to_vec())).unwrap();
    sender.send((Stream::Stdout, b"partial".to_vec())).unwrap();
    drop(sender);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths(&dir)),
        &mut Retain,
        &mut out,
        &mut err,
    );
    assert_eq!(out, b"partial");
    assert_eq!(err, b"\0\xff");
    assert!(report.raw_id.is_none());
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    assert!(metadata.is_empty());
    assert!(!paths(&dir).0[0].exists());
}

#[test]
fn ordinary_cli_passthrough_has_no_capture_or_metadata() {
    let dir = TestDir::new();
    let state = dir.path().join("state");
    let output = ttc_command()
        .args([
            "/bin/sh",
            "-c",
            "printf 'raw\\n'; printf 'diagnostic\\n' >&2",
        ])
        .env("HOME", dir.path())
        .env("XDG_STATE_HOME", &state)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"raw\n");
    assert_eq!(output.stderr, b"diagnostic\n");
    assert!(!state.join("ttc/runs").exists());
}
