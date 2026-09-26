mod common;

use common::{TestDir, ttc_command};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

fn make_tool(dir: &TestDir, name: &str, script: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn compare_once(path: &std::path::Path, args: &[&str], output_prefix: &str, reduce: bool) {
    let dir = TestDir::new();
    let direct_count = dir.path().join("direct.count");
    let wrapped_count = dir.path().join("wrapped.count");
    let mut direct = Command::new(path);
    direct
        .args(args)
        .current_dir(dir.path())
        .env("COUNT_FILE", &direct_count)
        .env("M5_MARKER", "m5-env");
    let baseline = direct.output().unwrap();
    let mut wrapped = ttc_command();
    wrapped
        .arg(path)
        .args(args)
        .current_dir(dir.path())
        .env("COUNT_FILE", &wrapped_count)
        .env("M5_MARKER", "m5-env");
    let result = wrapped.output().unwrap();
    assert_eq!(result.status, baseline.status);
    assert_eq!(fs::read(direct_count).unwrap(), b"x");
    assert_eq!(fs::read(wrapped_count).unwrap(), b"x");
    assert!(
        String::from_utf8_lossy(&result.stdout).contains(output_prefix),
        "expected {output_prefix:?} in {:?}",
        String::from_utf8_lossy(&result.stdout)
    );
    if output_prefix == "cwd=" {
        assert!(
            String::from_utf8_lossy(&result.stdout)
                .contains(&format!("cwd={} env=m5-env", dir.path().display()))
        );
    }
    assert!(result.stderr.starts_with(&baseline.stderr));
    if reduce {
        assert!(result.stdout.len() < baseline.stdout.len());
    } else {
        assert_eq!(result.stdout, baseline.stdout);
        assert_eq!(result.stderr, baseline.stderr);
    }
}

#[test]
fn rust_python_and_go_commands_run_once_keep_status_and_retain_diagnostics() {
    let dir = TestDir::new();
    let rust = make_tool(
        &dir,
        "cargo",
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf 'cwd=%s env=%s\\n' \"$PWD\" \"$M5_MARKER\"\ni=0\nwhile [ $i -lt 20 ]; do printf 'test fixture::case_%s ... ok\\n' \"$i\"; i=$((i+1)); done\nprintf 'warning: keep-warning\\n' >&2\nexit 7\n",
    );
    let python = make_tool(
        &dir,
        "pytest",
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\ni=0\nwhile [ $i -lt 20 ]; do printf 'tests/test_many.py::test_%s PASSED\\n' \"$i\"; i=$((i+1)); done\nprintf '20 passed, 1 warning\\n'\nexit 0\n",
    );
    let go = make_tool(
        &dir,
        "go",
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\ni=0\nwhile [ $i -lt 20 ]; do printf -- '--- PASS: Test%s (0.00s)\\n' \"$i\"; i=$((i+1)); done\nprintf 'ok\\texample/pkg\\t0.01s\\n'\nprintf 'panic: keep-panic\\n' >&2\nexit 0\n",
    );
    let app = make_tool(
        &dir,
        "python3",
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf 'app raw output: %s\\n' \"$M5_MARKER\"\nexit 0\n",
    );
    compare_once(&rust, &["test"], "cwd=", true);
    compare_once(&python, &["-q"], "20 passed", true);
    compare_once(&go, &["test", "-v", "./..."], "ok\texample/pkg", true);
    compare_once(&app, &["app.py"], "app raw output: m5-env", false);
}

#[test]
fn filtered_go_command_preserves_signal_status() {
    let dir = TestDir::new();
    let tool = make_tool(
        &dir,
        "go",
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nfor test in One Two Three Four; do printf -- '--- PASS: Test%s (0.00s)\\n' \"$test\"; done\nkill -TERM $$\n",
    );
    let direct_count = dir.path().join("direct.count");
    let wrapped_count = dir.path().join("wrapped.count");
    let direct = Command::new(&tool)
        .arg("test")
        .env("COUNT_FILE", &direct_count)
        .output()
        .unwrap();
    let wrapped = ttc_command()
        .arg(&tool)
        .arg("test")
        .env("COUNT_FILE", &wrapped_count)
        .output()
        .unwrap();
    assert_eq!(direct.status.signal(), Some(15));
    assert_eq!(wrapped.status.signal(), direct.status.signal());
    assert_eq!(fs::read(direct_count).unwrap(), b"x");
    assert_eq!(fs::read(wrapped_count).unwrap(), b"x");
    assert!(wrapped.stdout.len() < direct.stdout.len());
}
