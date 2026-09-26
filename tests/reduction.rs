mod common;

use common::{TestDir, ttc_command};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn large_output_is_reduced_and_raw_id_replays_original_bytes() {
    let dir = TestDir::new();
    let tool = dir.path().join("vitest");
    fs::write(&tool, b"#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 1001 ]; do printf 'PASS tests/long_repeating_case_%04d.test.js\\n' \"$i\"; i=$((i+1)); done\nprintf 'warning: original diagnostic\\n' >&2\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let baseline = Command::new(&tool).output().unwrap();
    let wrapped = ttc_command()
        .arg(&tool)
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .output()
        .unwrap();
    assert_eq!(wrapped.status, baseline.status);
    assert!(wrapped.stdout.len() * 5 < baseline.stdout.len());
    assert!(wrapped.stderr.starts_with(&baseline.stderr));
    let metadata = String::from_utf8_lossy(&wrapped.stderr);
    let passing = metadata
        .lines()
        .find_map(|line| line.strip_prefix("TTC: "))
        .and_then(|line| line.split_whitespace().next())
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(
        passing,
        1001 - wrapped
            .stdout
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .count()
    );
    assert!(passing >= 990);
    let id = metadata
        .lines()
        .find_map(|line| line.strip_prefix("raw: ttc raw "))
        .unwrap();
    let replay = ttc_command()
        .args(["raw", id])
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .output()
        .unwrap();
    assert!(replay.status.success());
    assert_eq!(replay.stdout, baseline.stdout);
    assert_eq!(replay.stderr, baseline.stderr);
}
