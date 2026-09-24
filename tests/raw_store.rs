#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use core::config::Config;
use core::raw_store::{Capture, StorePaths, Stream};
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn bounded_capture_preserves_latest_bytes_and_counts_discarded_payload() {
    let dir = TestDir::new();
    let state = dir.path().join("state");
    let roots = StorePaths([state.join("ttc/runs"), dir.path().join("temp/ttc/runs")]);
    let mut capture = Capture::new(Config::default(), roots.clone());
    let mut original = vec![b'a'; 34 * 1024 * 1024];
    let end = original.len();
    original[end - 3..].copy_from_slice(b"XYZ");
    capture.record(Stream::Stdout, &original[..1024]).unwrap();
    let id = capture.enable(Config::default()).unwrap().to_owned();
    capture.record(Stream::Stdout, &original[1024..]).unwrap();
    capture.finish().unwrap();
    let path = roots.0[0].join(format!("{id}.raw"));
    assert_eq!(fs::metadata(&path).unwrap().len(), 32 * 1024 * 1024);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&roots.0[0]).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let output = ttc_command()
        .args(["raw", &id, "--stdout"])
        .env("HOME", dir.path())
        .env("XDG_STATE_HOME", &state)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.len() < 32 * 1024 * 1024);
    assert_eq!(
        output.stdout,
        original[original.len() - output.stdout.len()..]
    );
    let dropped = String::from_utf8(output.stderr)
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(dropped + output.stdout.len(), original.len());
}

#[test]
fn no_compaction_creates_no_capture_file() {
    let dir = TestDir::new();
    let roots = StorePaths([
        dir.path().join("state/ttc/runs"),
        dir.path().join("temp/ttc/runs"),
    ]);
    let mut capture = Capture::new(Config::default(), roots.clone());
    capture.record(Stream::Stdout, b"raw\n").unwrap();
    assert!(capture.finish().unwrap().is_none());
    assert!(!roots.0[0].exists());
    assert!(!roots.0[1].exists());
}
