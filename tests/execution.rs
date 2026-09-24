mod common;

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use common::{TestDir, ttc_command};

fn with_input(mut command: Command, input: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn command");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .expect("write input");
    child.wait_with_output().expect("wait for command")
}

#[test]
fn direct_argv_runs_once_and_inherits_context() {
    let dir = TestDir::new();
    let baseline_count = dir.path().join("baseline-count");
    let wrapped_count = dir.path().join("wrapped-count");
    let script = "printf x >> \"$COUNT_FILE\"; pwd -P; printf '%s\\n' \"$M2_VALUE\"; cat; printf 'diagnostic\\n' >&2";

    let mut baseline = Command::new("/bin/sh");
    baseline
        .args(["-c", script])
        .current_dir(dir.path())
        .env("COUNT_FILE", &baseline_count)
        .env("M2_VALUE", "nilai-unicode-λ");
    let baseline = with_input(baseline, b"stdin\0bytes\n");

    let mut wrapped = ttc_command();
    wrapped
        .args(["/bin/sh", "-c", script])
        .current_dir(dir.path())
        .env("COUNT_FILE", &wrapped_count)
        .env("M2_VALUE", "nilai-unicode-λ");
    let wrapped = with_input(wrapped, b"stdin\0bytes\n");

    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(wrapped.stdout, baseline.stdout);
    assert_eq!(wrapped.stderr, baseline.stderr);
    assert_eq!(fs::read(baseline_count).unwrap(), b"x");
    assert_eq!(fs::read(wrapped_count).unwrap(), b"x");
}

#[test]
fn direct_argv_preserves_exit_codes() {
    for code in [0, 7] {
        let script = format!("exit {code}");
        let baseline = Command::new("/bin/sh")
            .args(["-c", &script])
            .output()
            .unwrap();
        let wrapped = ttc_command()
            .args(["/bin/sh", "-c", &script])
            .output()
            .unwrap();
        assert_eq!(wrapped.status, baseline.status);
        assert_eq!(wrapped.stdout, baseline.stdout);
        assert_eq!(wrapped.stderr, baseline.stderr);
    }
}

#[test]
fn forwarding_failure_never_reruns_the_command() {
    let dir = TestDir::new();
    let count_file = dir.path().join("invocations");
    let mut child = ttc_command()
        .args([
            "/bin/sh",
            "-c",
            "printf x >> \"$COUNT_FILE\"; printf 'output\\n'",
        ])
        .env("COUNT_FILE", &count_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let _ = child.wait().unwrap();
    assert_eq!(fs::read(count_file).unwrap(), b"x");
}
