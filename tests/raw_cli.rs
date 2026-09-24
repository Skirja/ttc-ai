#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use core::config::Config;
use core::raw_store::{Capture, StorePaths, Stream};
use std::fs::{self, File};
use std::process::Stdio;

#[test]
fn replay_selectors_and_byte_tail_follow_observed_events() {
    let dir = TestDir::new();
    let state = dir.path().join("state");
    let roots = StorePaths([state.join("ttc/runs"), dir.path().join("temp/ttc/runs")]);
    let mut capture = Capture::new(Config::default(), roots);
    capture.record(Stream::Stdout, b"ab\0").unwrap();
    capture.record(Stream::Stderr, b"XY").unwrap();
    capture.record(Stream::Stdout, b"cd").unwrap();
    let id = capture.enable(Config::default()).unwrap().to_owned();
    capture.finish().unwrap();
    let run = |args: &[&str]| {
        ttc_command()
            .arg("raw")
            .arg(&id)
            .args(args)
            .env("HOME", dir.path())
            .env("XDG_STATE_HOME", &state)
            .output()
            .unwrap()
    };
    let all = run(&[]);
    assert!(all.status.success());
    assert_eq!(all.stdout, b"ab\0cd");
    assert_eq!(all.stderr, b"XY");
    let merged_path = dir.path().join("merged");
    let merged = File::create(&merged_path).unwrap();
    let status = ttc_command()
        .args(["raw", &id])
        .env("HOME", dir.path())
        .env("XDG_STATE_HOME", &state)
        .stdout(Stdio::from(merged.try_clone().unwrap()))
        .stderr(Stdio::from(merged))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(fs::read(merged_path).unwrap(), b"ab\0XYcd");
    let stdout = run(&["--stdout"]);
    assert_eq!(stdout.stdout, b"ab\0cd");
    assert!(stdout.stderr.is_empty());
    let stderr = run(&["--stderr"]);
    assert_eq!(stderr.stdout, b"XY");
    assert!(stderr.stderr.is_empty());
    let tail = run(&["--tail", "3"]);
    assert_eq!(tail.stdout, b"cd");
    assert_eq!(tail.stderr, b"Y");
    let selected_tail = run(&["--stdout", "--tail", "2"]);
    assert_eq!(selected_tail.stdout, b"cd");
    assert!(run(&["--tail", "0"]).stdout.is_empty());
    assert_eq!(run(&["--stdout", "--stderr"]).status.code(), Some(2));
}

#[test]
fn invalid_id_is_rejected_without_path_access() {
    let output = ttc_command().args(["raw", "../secret"]).output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
