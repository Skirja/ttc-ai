#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

fn fake_tool(dir: &TestDir, name: &str) {
    let path = dir.path().join(name);
    fs::write(
        &path,
        "#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\nprintf '\\377\\000raw\\n'\nprintf 'warning: retain on stderr\\n' >&2\nexit 13\n",
    ).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn invoke(tool: &str, args: &[&str], dir: &TestDir, count: &str, wrapped: bool) -> Output {
    let mut command = if wrapped {
        ttc_command()
    } else {
        Command::new(tool)
    };
    if wrapped {
        command.arg(tool);
    }
    command
        .args(args)
        .current_dir(dir.path())
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.path().display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("COUNT_FILE", dir.path().join(count))
        .output()
        .unwrap()
}

#[test]
fn always_raw_readers_network_database_watch_and_infrastructure_commands_match_baseline() {
    let cases: &[(&str, &[&str])] = &[
        ("cat", &["README.md"]),
        ("bat", &["README.md"]),
        ("sed", &["-n", "1p", "README.md"]),
        ("awk", &["1", "README.md"]),
        ("head", &["README.md"]),
        ("tail", &["-f", "log.txt"]),
        ("less", &["README.md"]),
        ("more", &["README.md"]),
        ("file", &["README.md"]),
        ("strings", &["program"]),
        ("base64", &["program"]),
        ("xxd", &["program"]),
        ("hexdump", &["program"]),
        ("env", &["--chdir", "/tmp"]),
        ("printenv", &["PATH"]),
        ("echo", &["hello"]),
        ("printf", &["%s", "hello"]),
        ("curl", &["https://example.invalid"]),
        ("wget", &["https://example.invalid"]),
        ("ssh", &["host"]),
        ("scp", &["source", "host:target"]),
        ("mysql", &["database"]),
        ("mariadb", &["database"]),
        ("psql", &["database"]),
        ("sqlite3", &["database"]),
        ("mongosh", &["database"]),
        ("redis-cli", &["PING"]),
        ("sqlcmd", &["-Q", "SELECT 1"]),
        ("pgcli", &["database"]),
        ("git", &["diff", "--stat"]),
        ("git", &["show", "HEAD"]),
        ("git", &["-C", "project", "diff", "--stat"]),
        ("git", &["--no-pager", "show", "HEAD"]),
        ("terraform", &["plan"]),
        ("terraform", &["show"]),
        (
            "docker",
            &["compose", "-f", "compose.yaml", "up", "--watch"],
        ),
        (
            "docker",
            &["--context", "remote", "compose", "--profile", "web", "up"],
        ),
        ("docker", &["compose", "run", "-it", "service"]),
        ("kubectl", &["logs", "pod/api", "--follow=true"]),
        (
            "kubectl",
            &["--context", "dev", "-n", "team", "logs", "pod/api", "-f"],
        ),
        (
            "kubectl",
            &[
                "--kubeconfig",
                "cluster.yaml",
                "--cache-dir",
                "/tmp/kube-cache",
                "exec",
                "pod/api",
                "-it",
                "--",
                "sh",
            ],
        ),
    ];
    let dir = TestDir::new();
    for (tool, args) in cases {
        fake_tool(&dir, tool);
        let direct = invoke(tool, args, &dir, "direct.count", false);
        let wrapped = invoke(tool, args, &dir, "wrapped.count", true);
        assert_eq!(
            wrapped.status.code(),
            direct.status.code(),
            "{tool} {args:?}"
        );
        assert_eq!(wrapped.stdout, direct.stdout, "{tool} {args:?}");
        assert_eq!(wrapped.stderr, direct.stderr, "{tool} {args:?}");
        assert_eq!(fs::read(dir.path().join("direct.count")).unwrap(), b"x");
        assert_eq!(fs::read(dir.path().join("wrapped.count")).unwrap(), b"x");
        fs::remove_file(dir.path().join("direct.count")).unwrap();
        fs::remove_file(dir.path().join("wrapped.count")).unwrap();
    }
}

#[test]
fn unknown_application_and_structured_output_stay_byte_exact() {
    let dir = TestDir::new();
    fake_tool(&dir, "custom-app");
    let direct = invoke("custom-app", &["--json"], &dir, "direct.count", false);
    let wrapped = invoke("custom-app", &["--json"], &dir, "wrapped.count", true);
    assert_eq!(wrapped.stdout, direct.stdout);
    assert_eq!(wrapped.stderr, direct.stderr);
    assert_eq!(wrapped.status.code(), direct.status.code());
    assert_eq!(fs::read(dir.path().join("direct.count")).unwrap(), b"x");
    assert_eq!(fs::read(dir.path().join("wrapped.count")).unwrap(), b"x");
}
