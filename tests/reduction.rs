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

#[test]
fn large_rust_python_and_go_records_reduce_by_at_least_eighty_percent() {
    let cases = [
        (
            "cargo",
            vec!["test"],
            "test fixture::a_very_long_case_name_that_repeats_safely ... ok\n",
        ),
        (
            "pytest",
            vec!["-v"],
            "tests/test_repetitive.py::test_a_very_long_case_name PASSED\n",
        ),
        (
            "go",
            vec!["test", "-v"],
            "--- PASS: TestARepeatedlyLongAndRecognizableName (0.00s)\n",
        ),
    ];
    for (name, args, record) in cases {
        let dir = TestDir::new();
        let tool = dir.path().join(name);
        fs::write(&tool, b"#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 1001 ]; do printf '%s' \"$M5_RECORD\"; i=$((i+1)); done\nprintf 'test result: summary retained\\n'\n").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        let direct = Command::new(&tool)
            .args(&args)
            .env("M5_RECORD", record)
            .output()
            .unwrap();
        let wrapped = ttc_command()
            .arg(&tool)
            .args(&args)
            .env("M5_RECORD", record)
            .env("XDG_STATE_HOME", dir.path().join("state"))
            .output()
            .unwrap();
        assert_eq!(wrapped.status, direct.status, "{name}");
        let direct_bytes = direct.stdout.len() + direct.stderr.len();
        let model_bytes = wrapped.stdout.len() + wrapped.stderr.len();
        assert!(
            model_bytes * 5 < direct_bytes,
            "{name}: {model_bytes} / {direct_bytes}"
        );
        let compacted = String::from_utf8_lossy(&wrapped.stderr)
            .lines()
            .find_map(|line| line.strip_prefix("TTC: "))
            .and_then(|line| line.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
            .expect("compaction report is present");
        println!(
            "M5 {name}: baseline_bytes={direct_bytes} model_bytes={model_bytes} compacted_records={compacted}"
        );
        assert!(
            String::from_utf8_lossy(&wrapped.stdout).contains("summary retained"),
            "{name}"
        );
        let metadata = String::from_utf8_lossy(&wrapped.stderr);
        let id = metadata
            .lines()
            .find_map(|line| line.strip_prefix("raw: ttc raw "))
            .unwrap_or_else(|| panic!("missing raw hint for {name}"));
        let replay = ttc_command()
            .args(["raw", id])
            .env("XDG_STATE_HOME", dir.path().join("state"))
            .output()
            .unwrap();
        assert_eq!(replay.stdout, direct.stdout, "{name}");
        assert_eq!(replay.stderr, direct.stderr, "{name}");
    }
}
