#![cfg(unix)]
use serde_json::json;
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};
use tempfile::tempdir;
use ttc_ai::{
    command::{NestedStageAttribution, StageKind},
    recovery::Store,
};
fn executable(p: &Path, s: &str) {
    std::fs::write(p, s).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}
fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ttc"))
        .args(args)
        .current_dir(dir)
        .env("TTC_DATA_DIR", dir.join("data"))
        .env("TTC_CONFIG", dir.join("none.toml"))
        .output()
        .unwrap()
}
fn recall_id(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .lines()
        .find_map(|line| {
            line.strip_prefix("raw: ttc recall ")
                .or_else(|| line.strip_prefix("raw (partial): ttc recall "))
        })
        .expect("recall id")
        .to_owned()
}
#[test]
fn exit_codes_and_failures_are_preserved() {
    for code in [0, 1, 2, 42, 254] {
        let d = tempdir().unwrap();
        let tool = d.path().join("cargo");
        executable(
            &tool,
            &format!(
                "#!/bin/sh\nprintf 'test example ... ok\\n'\nprintf 'error: diagnosis retained\\n' >&2\nexit {code}\n"
            ),
        );
        let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
        assert_eq!(out.status.code(), Some(code));
        assert!(String::from_utf8_lossy(&out.stderr).contains("diagnosis retained"));
        let id = recall_id(&out.stderr);
        let raw = run(d.path(), &["recall", &id, "--raw"]);
        assert!(raw.status.success());
        assert!(String::from_utf8_lossy(&raw.stdout).contains("test example ... ok"));
        assert!(String::from_utf8_lossy(&raw.stdout).contains("diagnosis retained"));
        let record = Store::at(d.path().join("data"))
            .unwrap()
            .record(&id)
            .unwrap();
        let evidence = record.execution_evidence.expect("runtime evidence");
        assert_eq!(evidence.stages.len(), 1);
        assert_eq!(
            evidence.nested_stage_attribution,
            NestedStageAttribution::Unavailable
        );
        let stage = &evidence.stages[0];
        assert_eq!(stage.kind, StageKind::TopLevelProcess);
        assert!(stage.started && stage.finished);
        assert_eq!(stage.outcome.as_ref().and_then(|o| o.code), Some(code));
        assert!(stage.stdout.bytes > 0);
        assert!(stage.stderr.bytes > 0);
    }
}
#[test]
fn original_package_manager_is_authoritative() {
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("package.json"),
        json!({"scripts":{"lint":"eslint . --max-warnings 0"}}).to_string(),
    )
    .unwrap();
    let tool = d.path().join("npm");
    executable(
        &tool,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > args\nprintf '%s' \"$SENTINEL\" > environment\npwd > cwd\nprintf 'original manager executed\\n'\nexit 2\n",
    );
    let out = Command::new(env!("CARGO_BIN_EXE_ttc"))
        .args([
            "run",
            "--",
            tool.to_str().unwrap(),
            "run",
            "lint",
            "--",
            "a b",
            "--fix",
        ])
        .env("SENTINEL", "retained")
        .env("TTC_DATA_DIR", d.path().join("data"))
        .env("TTC_CONFIG", d.path().join("none"))
        .current_dir(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        std::fs::read_to_string(d.path().join("args")).unwrap(),
        "run\nlint\n--\na b\n--fix\n"
    );
    assert_eq!(
        std::fs::read_to_string(d.path().join("environment")).unwrap(),
        "retained"
    );
    assert_eq!(
        std::fs::read_to_string(d.path().join("cwd"))
            .unwrap()
            .trim(),
        d.path().to_str().unwrap()
    );
}
#[test]
fn shell_operators_are_executed_by_original_shell() {
    let d = tempdir().unwrap();
    let out = run(
        d.path(),
        &[
            "run",
            "--shell",
            "/bin/sh",
            "--command",
            "printf first; false && printf wrong; printf last",
        ],
    );
    assert!(out.status.success());
    assert_eq!(out.stdout, b"firstlast");
}
#[test]
fn unknown_command_output_is_exact() {
    let d = tempdir().unwrap();
    let out = run(d.path(), &["run", "--", "/bin/printf", "%s", "a b\n'\"$()"]);
    assert_eq!(out.stdout, b"a b\n'\"$()");
    assert!(out.stderr.is_empty());
}
#[test]
fn redirection_is_not_filtered() {
    let d = tempdir().unwrap();
    let out = run(
        d.path(),
        &[
            "run",
            "--shell",
            "/bin/sh",
            "--command",
            "printf 'test a ... ok\\n' > artifact",
        ],
    );
    assert!(out.status.success());
    assert_eq!(
        std::fs::read(d.path().join("artifact")).unwrap(),
        b"test a ... ok\n"
    );
}
#[test]
fn capture_limit_forwards_all_output_once() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join(".ttc.toml"), "[recovery]\nmax_capture_mb=0\n").unwrap();
    let tool = d.path().join("cargo");
    executable(
        &tool,
        "#!/bin/sh\nprintf 'test a ... ok\\nwarning: keep\\n'\nexit 42\n",
    );
    let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
    assert_eq!(out.status.code(), Some(42));
    assert_eq!(out.stdout, b"test a ... ok\nwarning: keep\n");
    let record = Store::at(d.path().join("data"))
        .unwrap()
        .record(&recall_id(&out.stderr))
        .unwrap();
    assert!(record.partial);
    let stage = &record.execution_evidence.unwrap().stages[0];
    assert_eq!(stage.stdout.bytes, out.stdout.len() as u64);
    assert!(stage.finished);
}
#[test]
fn true_signal_and_numeric_254_remain_distinct() {
    use std::os::unix::process::ExitStatusExt;
    let d = tempdir().unwrap();
    let tool = d.path().join("cargo");
    executable(&tool, "#!/bin/sh\nkill -TERM $$\n");
    let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
    assert_eq!(out.status.signal(), Some(15));
    let record = Store::at(d.path().join("data"))
        .unwrap()
        .record(&recall_id(&out.stderr))
        .unwrap();
    let outcome = record.execution_evidence.unwrap().stages[0]
        .outcome
        .clone()
        .unwrap();
    assert_eq!(outcome.signal, Some(15));
    assert_eq!(outcome.code, None);
}

#[test]
fn no_output_failure_has_completed_zero_stream_evidence() {
    let d = tempdir().unwrap();
    let tool = d.path().join("cargo");
    executable(&tool, "#!/bin/sh\nexit 7\n");
    let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
    assert_eq!(out.status.code(), Some(7));
    let record = Store::at(d.path().join("data"))
        .unwrap()
        .record(&recall_id(&out.stderr))
        .unwrap();
    let stage = &record.execution_evidence.unwrap().stages[0];
    assert!(stage.started && stage.finished);
    assert_eq!(stage.stdout.bytes, 0);
    assert_eq!(stage.stderr.bytes, 0);
    assert_eq!(stage.stdout.events, 0);
    assert_eq!(stage.stderr.events, 0);
}

#[test]
fn resolved_composite_workload_does_not_fabricate_nested_stages() {
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("package.json"),
        json!({"scripts":{"test":"cargo test && cargo test"}}).to_string(),
    )
    .unwrap();
    let tool = d.path().join("npm");
    executable(
        &tool,
        "#!/bin/sh\nprintf 'test example ... ok\\n'\nexit 7\n",
    );
    let out = run(
        d.path(),
        &["run", "--", tool.to_str().unwrap(), "run", "test"],
    );
    assert_eq!(out.status.code(), Some(7));
    let record = Store::at(d.path().join("data"))
        .unwrap()
        .record(&recall_id(&out.stderr))
        .unwrap();
    let evidence = record.execution_evidence.unwrap();
    assert_eq!(evidence.stages.len(), 1);
    assert_eq!(
        evidence.nested_stage_attribution,
        NestedStageAttribution::Unavailable
    );
}
#[test]
fn stderr_and_stdout_recall_selectors() {
    let d = tempdir().unwrap();
    let tool = d.path().join("cargo");
    executable(
        &tool,
        "#!/bin/sh\nprintf 'test a ... ok\\nlast'\nprintf 'warning: stderr-only' >&2\nexit 2\n",
    );
    let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
    let s = String::from_utf8_lossy(&out.stderr);
    let id = s
        .lines()
        .find_map(|l| l.strip_prefix("raw: ttc recall "))
        .unwrap();
    assert_eq!(
        run(d.path(), &["recall", id, "--stdout", "--raw"]).stdout,
        b"test a ... ok\nlast"
    );
    assert_eq!(
        run(d.path(), &["recall", id, "--stderr", "--raw"]).stdout,
        b"warning: stderr-only"
    );
}
#[test]
fn recovery_disabled_is_exact_passthrough() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join(".ttc.toml"), "[recovery]\nenabled=false\n").unwrap();
    let tool = d.path().join("cargo");
    executable(&tool, "#!/bin/sh\nprintf 'test a ... ok\\n'\n");
    let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
    assert_eq!(out.stdout, b"test a ... ok\n");
    assert!(out.stderr.is_empty());
}

#[test]
fn diagnostics_stream_before_child_finishes() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    let d = tempdir().unwrap();
    let tool = d.path().join("cargo");
    executable(
        &tool,
        "#!/bin/sh\nprintf 'error: ready for input\\n' >&2\nread answer\nexit 2\n",
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_ttc"))
        .args(["run", "--", tool.to_str().unwrap(), "test"])
        .current_dir(d.path())
        .env("TTC_DATA_DIR", d.path().join("data"))
        .env("TTC_CONFIG", d.path().join("none"))
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut r = BufReader::new(stderr);
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        tx.send(line).unwrap();
        let mut rest = String::new();
        std::io::Read::read_to_string(&mut r, &mut rest).unwrap();
    });
    let first = rx.recv_timeout(std::time::Duration::from_secs(5));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    let status = child.wait().unwrap();
    reader.join().unwrap();
    assert_eq!(first.unwrap(), "error: ready for input\n");
    assert_eq!(status.code(), Some(2));
}
#[test]
fn unusable_storage_never_changes_execution() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("data"), "not a directory").unwrap();
    let tool = d.path().join("cargo");
    executable(&tool, "#!/bin/sh\nprintf 'test a ... ok\\n'\nexit 42\n");
    let out = run(d.path(), &["run", "--", tool.to_str().unwrap(), "test"]);
    assert_eq!(out.status.code(), Some(42));
    assert_eq!(out.stdout, b"test a ... ok\n");
}
