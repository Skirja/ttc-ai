#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::{TestDir, ttc_command};
use core::classification::{Family, ManifestHints, classify};
use core::filters::DispatchFilter;
use core::raw_store::Stream;
use core::streaming::{CompactKind, Filter};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn pnpm_recursive_command_uses_workspace_scripts_and_path_prefixes() {
    let dir = TestDir::new();
    let root = dir.path();
    fs::create_dir_all(root.join("packages/api")).unwrap();
    fs::create_dir_all(root.join("packages/web")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"root","private":true,"workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - packages/*\n",
    )
    .unwrap();
    for name in ["api", "web"] {
        fs::write(
            root.join(format!("packages/{name}/package.json")),
            format!(
                r#"{{"name":"@m6/{name}","scripts":{{"test":"vitest run --reporter=verbose"}}}}"#
            ),
        )
        .unwrap();
    }
    let hints = ManifestHints::discover(root).unwrap().unwrap();
    let plan = classify(&["pnpm -r test".into()], root, Some(&hints));
    assert!(!plan.raw, "{plan:?}");
    assert_eq!(plan.families, vec![Family::Test], "{plan:?}");
    assert!(
        plan.source_aliases
            .iter()
            .any(|(alias, _)| alias == "packages/api")
    );
    let mut filter = DispatchFilter::new(plan.clone());
    for index in 0..3 {
        let record = format!("packages/api test:  ✓ tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    let record = b"packages/api test:  \xE2\x9C\x93 tests/case-3.test.js\n";
    assert!(matches!(
        filter.decide(Stream::Stdout, record).unwrap(),
        Some(CompactKind::Passing)
    ));
    let mut nx_filter = DispatchFilter::new(plan.clone());
    for index in 0..3 {
        let record = format!("@m6/api:  ✓ tests/case-{index}.test.js\n");
        assert!(
            nx_filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        nx_filter
            .decide(
                Stream::Stdout,
                b"@m6/api:  \xE2\x9C\x93 tests/case-3.test.js\n"
            )
            .unwrap(),
        Some(CompactKind::Passing)
    ));
    let mut lage_filter = DispatchFilter::new(plan);
    for index in 0..3 {
        let record = format!("@m6/api test :   ✓ tests/case-{index}.test.js\n");
        assert!(
            lage_filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        lage_filter
            .decide(
                Stream::Stdout,
                b"@m6/api test :   \xE2\x9C\x93 tests/case-3.test.js\n"
            )
            .unwrap(),
        Some(CompactKind::Passing)
    ));

    fs::create_dir_all(root.join("bin")).unwrap();
    let runner = root.join("bin/pnpm");
    fs::write(
        &runner,
        "#!/bin/sh\nprintf 'Scope: 2 of 3 workspace projects\\n'\nfor package in api web; do index=0; while [ $index -lt 8 ]; do printf 'packages/%s test:  ✓ tests/case-%s.test.js\\n' \"$package\" \"$index\"; index=$((index+1)); done; done\n",
    )
    .unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    let output = ttc_command()
        .arg("pnpm -r test")
        .current_dir(root)
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("10 passing records"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn confidence_isolated_between_tasks_for_the_same_project() {
    let dir = TestDir::new();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"api","scripts":{"test":"vitest run","lint":"eslint ."}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(&["npm run test".into()], dir.path(), Some(&hints));
    let mut filter = DispatchFilter::new(plan);
    for index in 0..3 {
        let record = format!("api#test: PASS tests/{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(
        filter
            .decide(Stream::Stdout, b"api#lint: PASS tests/other.test.js\n")
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        filter
            .decide(Stream::Stdout, b"api#test: PASS tests/four.test.js\n")
            .unwrap(),
        Some(CompactKind::Passing)
    ));
}

#[test]
fn multi_project_runner_requires_a_registered_prefix() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::create_dir_all(dir.path().join("packages/web")).unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(dir.path().join("turbo.json"), r#"{"tasks":{"test":{}}}"#).unwrap();
    for name in ["api", "web"] {
        fs::write(
            dir.path().join(format!("packages/{name}/package.json")),
            format!(r#"{{"name":"{name}","scripts":{{"test":"vitest run"}}}}"#),
        )
        .unwrap();
    }
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(&["turbo run test".into()], dir.path(), Some(&hints));
    assert!(plan.fallback_only_prefixed);
    let mut filter = DispatchFilter::new(plan);
    for index in 0..5 {
        let record = format!("PASS tests/unprefixed-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    for index in 0..3 {
        let record = format!("api#test: PASS tests/prefixed-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter
            .decide(Stream::Stdout, b"api#test: PASS tests/prefixed-3.test.js\n")
            .unwrap(),
        Some(CompactKind::Passing)
    ));
}

#[test]
fn warning_text_after_a_project_prefix_is_never_parsed_as_a_task() {
    let dir = TestDir::new();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"api","scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(&["npm run test".into()], dir.path(), Some(&hints));
    let mut filter = DispatchFilter::new(plan);

    for index in 0..8 {
        let warning = format!("api: warning: PASS tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, warning.as_bytes())
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn turbo_prefixed_records_compact_per_package_and_root_command_runs_once() {
    let dir = TestDir::new();
    let root = dir.path();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("packages/api")).unwrap();
    fs::create_dir_all(root.join("packages/web")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"],"scripts":{"test":"turbo run test"}}"#,
    )
    .unwrap();
    fs::write(
        root.join("turbo.json"),
        r#"{"tasks":{"test":{"dependsOn":["^test"]}}}"#,
    )
    .unwrap();
    for name in ["api", "web"] {
        fs::write(
            root.join(format!("packages/{name}/package.json")),
            format!(r#"{{"name":"{name}","scripts":{{"test":"vitest run"}}}}"#),
        )
        .unwrap();
    }
    let fake = root.join("bin/pnpm");
    fs::write(
        &fake,
        r##"#!/bin/sh
printf x >> "$ROOT_MARKER"
for package in packages/*; do
  name=${package#packages/}
  printf x >> "$CHILD_MARKER/$name"
  index=0
  while [ "$index" -lt 1000 ]; do
    printf 'packages/%s test:  PASS tests/case-%s.test.js\n' "$name" "$index"
    index=$((index + 1))
  done
done
if [ "${FAIL_TESTS:-0}" = 1 ]; then
  printf 'Error: package assertion failed\n\nexpected 2, actual 3\n' >&2
  exit 9
fi
"##,
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let direct_root = root.join("direct-root");
    let wrapped_root = root.join("wrapped-root");
    let direct_children = root.join("direct-children");
    let wrapped_children = root.join("wrapped-children");
    fs::create_dir(&direct_children).unwrap();
    fs::create_dir(&wrapped_children).unwrap();
    let direct = Command::new(&fake)
        .args(["run", "test"])
        .current_dir(root)
        .env("ROOT_MARKER", &direct_root)
        .env("CHILD_MARKER", &direct_children)
        .output()
        .unwrap();
    let wrapped = ttc_command()
        .arg("pnpm run test")
        .current_dir(root)
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("ROOT_MARKER", &wrapped_root)
        .env("CHILD_MARKER", &wrapped_children)
        .output()
        .unwrap();
    assert_eq!(wrapped.status, direct.status);
    assert_eq!(fs::read(direct_root).unwrap(), b"x");
    assert_eq!(fs::read(wrapped_root).unwrap(), b"x");
    for name in ["api", "web"] {
        assert_eq!(fs::read(direct_children.join(name)).unwrap(), b"x");
        assert_eq!(fs::read(wrapped_children.join(name)).unwrap(), b"x");
    }
    assert_eq!(
        direct.stdout.iter().filter(|byte| **byte == b'\n').count(),
        2000
    );
    let direct_total = direct.stdout.len() + direct.stderr.len();
    let wrapped_total = wrapped.stdout.len() + wrapped.stderr.len();
    assert!(
        wrapped_total * 5 < direct_total,
        "direct total={direct_total}, TTC total={wrapped_total}"
    );
    let reduction = (direct_total - wrapped_total) as f64 * 100.0 / direct_total as f64;
    println!(
        "M6_LARGE_MONOREPO direct_records=2000 root_invocation=1 package_invocations=2 direct_total={direct_total} ttc_total={wrapped_total} reduction_percent={reduction:.1}"
    );
    assert!(String::from_utf8_lossy(&wrapped.stderr).contains("1994 passing records"));

    let failed_root = root.join("failed-root");
    let failed_children = root.join("failed-children");
    fs::create_dir(&failed_children).unwrap();
    let failed = ttc_command()
        .arg("pnpm run test")
        .current_dir(root)
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("ROOT_MARKER", &failed_root)
        .env("CHILD_MARKER", &failed_children)
        .env("FAIL_TESTS", "1")
        .output()
        .unwrap();
    let baseline_failure = Command::new(&fake)
        .args(["run", "test"])
        .current_dir(root)
        .env("ROOT_MARKER", root.join("baseline-failed-root"))
        .env("CHILD_MARKER", &direct_children)
        .env("FAIL_TESTS", "1")
        .output()
        .unwrap();
    assert_eq!(baseline_failure.status.code(), Some(9));
    assert_eq!(failed.status.code(), Some(9));
    let failed_err = String::from_utf8_lossy(&failed.stderr);
    assert!(failed_err.contains("Error: package assertion failed"));
    assert!(failed_err.contains("expected 2, actual 3"));
    assert!(failed.stderr.starts_with(&baseline_failure.stderr));
    assert_eq!(fs::read(failed_root).unwrap(), b"x");
}
