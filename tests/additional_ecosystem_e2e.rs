mod common;

use common::{TestDir, ttc_command};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output, Stdio};

fn make_tool(dir: &TestDir, name: &str) {
    let path = dir.path().join(name);
    fs::write(
        &path,
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf '%s' \"$M7_OUTPUT\"\nprintf 'warning: preserve this\\n' >&2\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn run(
    name: &str,
    args: &[&str],
    dir: &TestDir,
    count: &str,
    output: &str,
    wrapped: bool,
) -> Output {
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut command = if wrapped {
        ttc_command()
    } else {
        Command::new(name)
    };
    if wrapped {
        command.arg(name);
    }
    command
        .args(args)
        .current_dir(dir.path())
        .env("PATH", path)
        .env("COUNT_FILE", dir.path().join(count))
        .env("M7_OUTPUT", output);
    command.output().unwrap()
}

fn make_context_tool(dir: &TestDir, name: &str) {
    let path = dir.path().join(name);
    fs::write(
        &path,
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf '%s|%s|stdin:' \"$M7_CONTEXT\" \"$PWD\" >&2\ncat >&2\nprintf '%s' \"$M7_OUTPUT\"\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn run_with_stdin(
    name: &str,
    args: &[&str],
    dir: &TestDir,
    count: &str,
    context: &str,
    output: &str,
    wrapped: bool,
) -> Output {
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap()
    );
    let mut command = if wrapped {
        ttc_command()
    } else {
        Command::new(name)
    };
    if wrapped {
        command.arg(name);
    }
    let mut child = command
        .args(args)
        .current_dir(dir.path())
        .env("PATH", path)
        .env("COUNT_FILE", dir.path().join(count))
        .env("M7_CONTEXT", context)
        .env("M7_OUTPUT", output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"task-input\n")
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn php_jvm_and_dotnet_tools_execute_once_preserve_status_and_keep_diagnostics() {
    let cases = vec![
        (
            "phpunit",
            vec![],
            "PHPUnit 13.3.4\nRuntime: PHP 8.4.25\nConfiguration: /tmp/phpunit.xml\nTime: 00:00.283, Memory: 24.00 MB\n".to_owned()
                + &"✔ successful test\n".repeat(20)
                + "error: preserve diagnostic\n",
            b"error: preserve diagnostic\n".as_slice(),
        ),
        (
            "mvn",
            vec!["test"],
            "[INFO] Downloading from central: https://repo.test/a.jar\n".repeat(20)
                + "error: preserve diagnostic\n",
            b"error: preserve diagnostic\n".as_slice(),
        ),
        (
            "gradle",
            vec!["test"],
            "> Task :module:test\n".repeat(20) + "error: preserve diagnostic\n",
            b"error: preserve diagnostic\n".as_slice(),
        ),
        (
            "dotnet",
            vec![
                "test",
                "--no-restore",
                "--no-build",
                "--verbosity",
                "normal",
                "--filter",
                "FullyQualifiedName!~FailureTests",
            ],
            "Build started 09/29/2026 10:07:47.\nTest run for /tmp/Tests.dll (.NETCoreApp,Version=v10.0)\nA total of 1 test files matched the specified pattern.\n".to_owned()
                + &"Passed Example.Tests.Add [4 ms]\n".repeat(20)
                + "Time Elapsed 00:00:01.55\nerror CS1001: preserve diagnostic\n",
            b"error CS1001: preserve diagnostic\n".as_slice(),
        ),
    ];
    for (name, args, output, diagnostic) in cases {
        let dir = TestDir::new();
        make_tool(&dir, name);
        let direct = run(name, &args, &dir, "direct.count", &output, false);
        let wrapped = run(name, &args, &dir, "wrapped.count", &output, true);
        assert_eq!(direct.status.code(), Some(7));
        assert_eq!(wrapped.status.code(), direct.status.code());
        assert_eq!(fs::read(dir.path().join("direct.count")).unwrap(), b"x");
        assert_eq!(fs::read(dir.path().join("wrapped.count")).unwrap(), b"x");
        assert!(wrapped.stdout.len() < direct.stdout.len(), "{name}");
        assert!(wrapped.stdout.ends_with(diagnostic));
        assert!(wrapped.stderr.starts_with(&direct.stderr));
        assert!(
            wrapped
                .stderr
                .windows(b"warning: preserve this".len())
                .any(|window| { window == b"warning: preserve this" })
        );
    }
}

#[test]
fn ecosystem_commands_preserve_cwd_environment_stdin_and_single_invocation_when_filtered() {
    let cases = vec![
        ("phpunit", vec![], "✔ successful test\n".repeat(20)),
        (
            "mvn",
            vec!["test"],
            "[INFO] Downloading from central: https://repo.test/a.jar\n".repeat(20),
        ),
        ("gradle", vec!["test"], "> Task :module:test\n".repeat(20)),
        (
            "dotnet",
            vec![
                "test",
                "--no-restore",
                "--no-build",
                "--verbosity",
                "normal",
                "--filter",
                "FullyQualifiedName!~FailureTests",
            ],
            "Passed Example.Tests.Add [4 ms]\n".repeat(20),
        ),
    ];
    for (name, args, output) in cases {
        let dir = TestDir::new();
        make_context_tool(&dir, name);
        let context = format!("m7-context-{name}");
        let direct = run_with_stdin(
            name,
            &args,
            &dir,
            "direct-context.count",
            &context,
            &output,
            false,
        );
        let wrapped = run_with_stdin(
            name,
            &args,
            &dir,
            "wrapped-context.count",
            &context,
            &output,
            true,
        );
        let expected_context = format!("{context}|{}|stdin:task-input\n", dir.path().display());
        assert_eq!(direct.status.code(), Some(7), "{name}");
        assert_eq!(wrapped.status.code(), direct.status.code(), "{name}");
        assert_eq!(direct.stderr, expected_context.as_bytes(), "{name}");
        assert!(
            wrapped.stderr.starts_with(expected_context.as_bytes()),
            "{name}"
        );
        assert_eq!(
            fs::read(dir.path().join("direct-context.count")).unwrap(),
            b"x",
            "{name} direct invocation count"
        );
        assert_eq!(
            fs::read(dir.path().join("wrapped-context.count")).unwrap(),
            b"x",
            "{name} wrapped invocation count"
        );
        assert!(wrapped.stdout.len() < direct.stdout.len(), "{name}");
    }
}

#[test]
fn generic_php_application_that_prints_pass_like_output_stays_byte_exact() {
    let dir = TestDir::new();
    make_tool(&dir, "php");
    let output = "....\n".repeat(10);
    let direct = run("php", &["app.php"], &dir, "direct.count", &output, false);
    let wrapped = run("php", &["app.php"], &dir, "wrapped.count", &output, true);
    assert_eq!(wrapped.stdout, direct.stdout);
    assert_eq!(wrapped.stderr, direct.stderr);
    assert_eq!(wrapped.status, direct.status);
}

#[test]
fn filtered_ecosystem_command_preserves_signal_and_executes_once() {
    use std::os::unix::process::ExitStatusExt;
    let dir = TestDir::new();
    let executable = dir.path().join("phpunit");
    fs::write(
        &executable,
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf '....\\n....\\n....\\n....\\n'\nkill -TERM $$\n",
    )
    .unwrap();
    fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap()
    );
    let direct = Command::new("phpunit")
        .arg("--colors=never")
        .current_dir(dir.path())
        .env("PATH", &path)
        .env("COUNT_FILE", dir.path().join("direct.count"))
        .output()
        .unwrap();
    let wrapped = ttc_command()
        // Two arguments select direct argv execution; a lone argument uses
        // the native shell, whose child-signal exit status is shell-specific.
        .args(["phpunit", "--colors=never"])
        .current_dir(dir.path())
        .env("PATH", &path)
        .env("COUNT_FILE", dir.path().join("wrapped.count"))
        .output()
        .unwrap();
    assert_eq!(direct.status.signal(), Some(15));
    assert_eq!(wrapped.status.signal(), direct.status.signal());
    assert_eq!(fs::read(dir.path().join("direct.count")).unwrap(), b"x");
    assert_eq!(fs::read(dir.path().join("wrapped.count")).unwrap(), b"x");
}
