mod common;

use common::{TestDir, ttc_command};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn wrapped_javascript_command_runs_once_and_preserves_diagnostics_and_exit() {
    let dir = TestDir::new();
    let tool = dir.path().join("vitest");
    fs::write(&tool, b"#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf 'PASS tests/one.test.js\\nPASS tests/two.test.js\\nPASS tests/three.test.js\\nPASS tests/four.test.js\\n'\nprintf 'warning: keep this\\n' >&2\nprintf 'Error: assertion failed\\n' >&2\nexit 7\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let direct_count = dir.path().join("direct-count");
    let wrapped_count = dir.path().join("wrapped-count");
    let baseline = Command::new(&tool)
        .env("COUNT_FILE", &direct_count)
        .output()
        .unwrap();
    let wrapped = ttc_command()
        .arg(&tool)
        .env("COUNT_FILE", &wrapped_count)
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .output()
        .unwrap();
    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(fs::read(direct_count).unwrap(), b"x");
    assert_eq!(fs::read(wrapped_count).unwrap(), b"x");
    assert_eq!(
        wrapped.stdout,
        b"PASS tests/one.test.js\nPASS tests/two.test.js\nPASS tests/three.test.js\n"
    );
    assert!(wrapped.stderr.starts_with(&baseline.stderr));
    assert!(
        String::from_utf8_lossy(&wrapped.stderr)
            .contains("1 passing records and 0 progress records compacted")
    );
}

#[test]
fn unknown_and_machine_readable_commands_match_baseline_exactly() {
    let dir = TestDir::new();
    let tool = dir.path().join("vitest");
    fs::write(
        &tool,
        b"#!/bin/sh\nprintf 'PASS a\\nPASS b\\nPASS c\\nPASS d\\n'\n",
    )
    .unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let baseline = Command::new(&tool).arg("--json").output().unwrap();
    let wrapped = ttc_command().arg(&tool).arg("--json").output().unwrap();
    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(wrapped.stdout, baseline.stdout);
    assert_eq!(wrapped.stderr, baseline.stderr);
}

fn assert_package_output_is_raw(manifest: &str, arguments: &[&str], bytes: &[u8]) {
    let dir = TestDir::new();
    fs::write(dir.path().join("package.json"), manifest).unwrap();
    let output_file = dir.path().join("output");
    fs::write(&output_file, bytes).unwrap();
    let tool = dir.path().join("npm");
    fs::write(&tool, b"#!/bin/sh\ncat \"$FIXTURE_OUTPUT\"\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let baseline = Command::new(&tool)
        .args(arguments)
        .current_dir(dir.path())
        .env("FIXTURE_OUTPUT", &output_file)
        .output()
        .unwrap();
    let wrapped = ttc_command()
        .arg(&tool)
        .args(arguments)
        .current_dir(dir.path())
        .env("FIXTURE_OUTPUT", &output_file)
        .env("XDG_STATE_HOME", dir.path().join("state"))
        .output()
        .unwrap();
    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(wrapped.stdout, baseline.stdout);
    assert_eq!(wrapped.stderr, baseline.stderr);
    assert!(!dir.path().join("state/ttc/runs").exists());
}

#[test]
fn selected_workspace_does_not_use_root_script_as_filter_hint() {
    assert_package_output_is_raw(
        r#"{"scripts":{"test":"vitest run"},"workspaces":["api"]}"#,
        &["--workspace", "api", "test"],
        "✓ custom value one\n✓ custom value two\n✓ custom value three\n✓ custom value four\n"
            .as_bytes(),
    );
}

#[test]
fn machine_readable_flag_inside_package_script_keeps_all_output() {
    assert_package_output_is_raw(
        r#"{"scripts":{"check":"vitest run --reporter=verbose && vitest run --reporter=json"}}"#,
        &["run", "check"],
        b"PASS tests/one.test.js\nPASS tests/two.test.js\nPASS tests/three.test.js\nPASS tests/four.test.js\n{\"tests\":4}\n",
    );
}
