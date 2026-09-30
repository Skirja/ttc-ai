#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};

fn install_fake(dir: &TestDir, name: &str, body: &str) {
    let path = dir.path().join(name);
    fs::write(
        path,
        format!("#!/bin/sh\nprintf x >> \"$COUNT_FILE\"\n{body}"),
    )
    .unwrap();
    fs::set_permissions(dir.path().join(name), fs::Permissions::from_mode(0o755)).unwrap();
}

fn run(name: &str, args: &[&str], dir: &TestDir, wrapped: bool, count: &str) -> Output {
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
fn build_ruby_swift_container_and_infrastructure_commands_match_status_and_invocation_count() {
    let cases: &[(&str, &[&str], &str, u8)] = &[
        (
            "cmake",
            &["--build", "build"],
            "printf '[ 10%%] Building CXX object demo.o\\n'; printf 'warning: preserved\\n' >&2; exit 4",
            4,
        ),
        (
            "ctest",
            &["--test-dir", "build"],
            "printf '1/1 Test #1: smoke ... Passed 0.01 sec\\n'; printf '100%% tests passed, 0 tests failed\\n'; exit 0",
            0,
        ),
        (
            "ninja",
            &[],
            "printf '[1/2] Linking CXX executable demo\\n'; exit 0",
            0,
        ),
        (
            "make",
            &["test"],
            "printf '1/1 Test #1: smoke ... Passed 0.01 sec\\n'; exit 0",
            0,
        ),
        (
            "rspec",
            &[],
            "printf '....\\n'; printf '1 example, 0 failures\\n'; exit 0",
            0,
        ),
        (
            "rubocop",
            &[],
            "printf '......\\n'; printf '1 file inspected, no offenses detected\\n'; exit 0",
            0,
        ),
        (
            "rake",
            &["test"],
            "printf '....\\n'; printf '4 runs, 0 failures\\n'; exit 0",
            0,
        ),
        (
            "swift",
            &["build"],
            "printf \"Compile Swift Module 'Demo' (1 sources)\\n\"; exit 0",
            0,
        ),
        (
            "swift",
            &["test"],
            "printf '✔ Test Demo.testPass() passed after 0.01 seconds.\\n'; exit 0",
            0,
        ),
        (
            "docker",
            &["build", "."],
            "printf '#1 [internal] load build definition from Dockerfile\\n'; printf '#2 [build 1/1] RUN echo output\\n'; exit 0",
            0,
        ),
        (
            "podman",
            &["build", "."],
            "printf 'STEP 1/1: FROM scratch\\n'; COMMIT demo; exit 0",
            0,
        ),
        (
            "terraform",
            &["validate"],
            "printf 'Success! The configuration is valid.\\n'; exit 0",
            0,
        ),
        (
            "helm",
            &["lint", "charts/demo"],
            "printf '==> Linting charts/demo\\n'; printf '1 chart(s) linted, 0 chart(s) failed\\n'; exit 0",
            0,
        ),
    ];
    let dir = TestDir::new();
    for (name, args, body, expected) in cases {
        install_fake(&dir, name, body);
        let direct = run(name, args, &dir, false, "direct.count");
        let wrapped = run(name, args, &dir, true, "wrapped.count");
        assert_eq!(
            direct.status.code(),
            Some(*expected as i32),
            "{name} {args:?}"
        );
        assert_eq!(
            wrapped.status.code(),
            direct.status.code(),
            "{name} {args:?}"
        );
        assert_eq!(fs::read(dir.path().join("direct.count")).unwrap(), b"x");
        assert_eq!(fs::read(dir.path().join("wrapped.count")).unwrap(), b"x");
        if *name == "terraform" || *name == "podman" {
            assert_eq!(wrapped.stdout, direct.stdout, "{name}");
        }
        if *name == "cmake" {
            assert!(wrapped.stderr.ends_with(b"warning: preserved\n"));
        }
        fs::remove_file(dir.path().join("direct.count")).unwrap();
        fs::remove_file(dir.path().join("wrapped.count")).unwrap();
    }
}
