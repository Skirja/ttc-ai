#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, ManifestHints, Plan, classify};
use core::filters::DispatchFilter;
use core::raw_store::Stream;
use core::streaming::{CompactKind, Filter};
use std::ffi::OsString;
use std::fs;

#[test]
fn mixed_js_go_and_nested_python_rust_scripts_accumulate_families() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/js")).unwrap();
    fs::write(dir.path().join("package.json"), r#"{"name":"root","workspaces":["packages/*"],"scripts":{"check":"turbo run test && go test ./...","nested":"npm run py && npm run rust","py":"uv run pytest","rust":"cargo test"}}"#).unwrap();
    fs::write(
        dir.path().join("packages/js/package.json"),
        r#"{"name":"js","scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();

    let mixed = classify(&[OsString::from("npm run check")], dir.path(), Some(&hints));
    assert!(mixed.families.contains(&Family::Test));
    assert!(mixed.families.contains(&Family::GoTest));
    assert!(!mixed.raw);

    let nested = classify(
        &[OsString::from("npm run nested")],
        dir.path(),
        Some(&hints),
    );
    assert!(nested.families.contains(&Family::PyTest));
    assert!(nested.families.contains(&Family::RustTest));
    assert!(!nested.raw);
}

#[test]
fn dynamic_nx_selection_uses_manifest_candidates_with_a_constrained_plan() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("nx.json"),
        r#"{"targetDefaults":{"test":{"cache":true}}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/api/package.json"),
        r#"{"name":"api","scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(
        &[OsString::from("nx affected -t test")],
        dir.path(),
        Some(&hints),
    );
    assert!(plan.families.contains(&Family::Test));
    assert!(plan.fallback);
    assert!(plan.sources.contains(&"api".to_owned()));
    assert!(!plan.raw);
    let shorthand = classify(&[OsString::from("nx test api")], dir.path(), Some(&hints));
    assert!(shorthand.families.contains(&Family::Test));
    assert_eq!(shorthand.sources, vec!["api"]);
    assert!(!shorthand.raw);
    let wrapped = classify(
        &[OsString::from("npx nx test api")],
        dir.path(),
        Some(&hints),
    );
    assert!(wrapped.families.contains(&Family::Test));
    assert_eq!(wrapped.sources, vec!["api"]);
    assert!(!wrapped.raw);
    let unsupported = classify(
        &[OsString::from("nx test api --unknown-runner-option")],
        dir.path(),
        Some(&hints),
    );
    assert!(unsupported.raw);
}

#[test]
fn static_runner_selectors_limit_script_evidence_to_selected_projects() {
    let dir = TestDir::new();
    for project in ["api", "web", "shared"] {
        fs::create_dir_all(dir.path().join(format!("packages/{project}"))).unwrap();
    }
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"],"scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("turbo.json"),
        r#"{"tasks":{"test":{"cache":true}}}"#,
    )
    .unwrap();
    for (project, script) in [
        ("api", "vitest run"),
        ("web", "node app.js"),
        ("shared", "jest"),
    ] {
        fs::write(
            dir.path().join(format!("packages/{project}/package.json")),
            format!(r#"{{"name":"@m6/{project}","scripts":{{"test":"{script}"}}}}"#),
        )
        .unwrap();
    }
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();

    for command in [
        "turbo run test --filter=@m6/api",
        "lerna run test --scope=@m6/api --ignore=@m6/web",
        "nx run-many -t test -p @m6/api --exclude @m6/web",
        "lage test --scope @m6/api",
    ] {
        let plan = classify(&[OsString::from(command)], dir.path(), Some(&hints));
        assert!(plan.families.contains(&Family::Test), "{command}: {plan:?}");
        assert!(!plan.raw, "{command}: {plan:?}");
        assert_eq!(plan.sources, vec!["@m6/api"], "{command}: {plan:?}");
    }

    let explicit_root = classify(
        &[OsString::from("turbo run test --filter=//")],
        dir.path(),
        Some(&hints),
    );
    assert!(explicit_root.families.contains(&Family::Test));
    assert!(!explicit_root.raw);
    assert_eq!(explicit_root.sources, vec!["root"]);

    let dynamic = classify(
        &[OsString::from("turbo run test --filter=...[origin/main]")],
        dir.path(),
        Some(&hints),
    );
    assert!(dynamic.fallback);
    assert!(dynamic.fallback_only_prefixed);
    assert!(dynamic.raw);

    let unknown_scope = classify(
        &[OsString::from("lerna run test --scope @m6/missing")],
        dir.path(),
        Some(&hints),
    );
    assert!(unknown_scope.raw);
}

#[test]
fn nx_project_json_targets_are_discovered_statically() {
    let dir = TestDir::new();
    fs::write(
        dir.path().join("nx.json"),
        r#"{"targetDefaults":{"test":{"cache":true}}}"#,
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("apps/api")).unwrap();
    fs::write(
        dir.path().join("apps/api/project.json"),
        r#"{"name":"api","root":"apps/api","targets":{"test":{"executor":"@nx/jest:jest"}}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(
        &[OsString::from("nx run api:test")],
        dir.path(),
        Some(&hints),
    );
    assert!(plan.families.contains(&Family::Test));
    assert!(plan.sources.contains(&"api".to_owned()));
    assert!(!plan.raw);
    let mut filter = DispatchFilter::new(plan);
    for index in 0..3 {
        let record = format!("api:test |  ✓ tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter
            .decide(
                Stream::Stdout,
                b"api:test |  \xE2\x9C\x93 tests/case-3.test.js\n"
            )
            .unwrap(),
        Some(CompactKind::Passing)
    ));
}

#[test]
fn nx_targets_do_not_replace_package_manager_scripts() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(dir.path().join("nx.json"), r#"{"targetDefaults":{}}"#).unwrap();
    fs::write(
        dir.path().join("packages/api/package.json"),
        r#"{"name":"api","scripts":{"test":"node app.js"}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/api/project.json"),
        r#"{"name":"api","root":"packages/api","targets":{"test":{"executor":"@nx/jest:jest"}}}"#,
    )
    .unwrap();

    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let npm = classify(
        &[OsString::from("npm --workspace api run test")],
        dir.path(),
        Some(&hints),
    );
    assert!(
        npm.raw,
        "a custom npm script must not inherit the Nx target: {npm:?}"
    );

    let nx = classify(
        &[OsString::from("nx run api:test")],
        dir.path(),
        Some(&hints),
    );
    assert!(nx.families.contains(&Family::Test));
    assert!(
        !nx.raw,
        "the Nx command should still use its own target: {nx:?}"
    );
}

#[test]
fn hidden_turbo_target_uses_known_prefixed_families_and_keeps_unprefixed_records() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("turbo.json"),
        r#"{"tasks":{"test":{"cache":true}}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/api/package.json"),
        r#"{"name":"api","scripts":{"custom":"node app.js"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(
        &[OsString::from("turbo run test")],
        dir.path(),
        Some(&hints),
    );
    assert!(plan.families.contains(&Family::Test));
    assert!(plan.fallback_only_prefixed);
    assert!(plan.sources.contains(&"api".to_owned()));
    assert!(!plan.raw);
}

#[test]
fn hidden_target_keeps_unprefixed_rust_records_raw() {
    let mut plan = Plan::default();
    plan.families.push(Family::RustTest);
    plan.fallback_only_prefixed = true;
    plan.source_aliases.push(("api".into(), "api".into()));
    let mut filter = DispatchFilter::new(plan);
    for index in 0..5 {
        let record = format!("test unprefixed_{index} ... ok\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    for index in 0..3 {
        let record = format!("api#test: test prefixed_{index} ... ok\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter
            .decide(Stream::Stdout, b"api#test: test prefixed_3 ... ok\n")
            .unwrap(),
        Some(CompactKind::Passing)
    ));
}

#[test]
fn moon_workspace_and_task_yaml_are_static_hints() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join(".moon/tasks")).unwrap();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/api/package.json"),
        r#"{"name":"@m6/api","scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join(".moon/workspace.yml"),
        "projects:\n  api: packages/api\n",
    )
    .unwrap();
    fs::write(
        dir.path().join(".moon/tasks/all.yml"),
        "tasks:\n  test:\n    command: vitest\n    args: [run]\n",
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(
        &[OsString::from("moon run :test")],
        dir.path(),
        Some(&hints),
    );
    assert!(plan.families.contains(&Family::Test));
    assert!(plan.sources.contains(&"@m6/api".to_owned()));
    assert!(
        plan.source_aliases
            .iter()
            .any(|(alias, source)| alias == "api" && source == "@m6/api")
    );
    assert!(!plan.raw);
}

#[test]
fn source_state_limit_disables_remaining_compaction_across_dispatchers() {
    let mut plan = Plan::default();
    plan.families.push(Family::Test);
    plan.source_aliases = (0..=4096)
        .map(|index| (format!("p{index}"), format!("p{index}")))
        .collect();
    let mut filter = DispatchFilter::new(plan);
    for index in 0..3 {
        let record = format!("p0#test:  ✓ tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    for index in 1..4096 {
        let record = format!("p{index}#test:  ✓ tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter
            .decide(
                Stream::Stdout,
                b"p0#test:  \xE2\x9C\x93 tests/case-3.test.js\n"
            )
            .unwrap(),
        Some(CompactKind::Passing)
    ));
    assert!(
        filter
            .decide(
                Stream::Stdout,
                b"p4096#test:  \xE2\x9C\x93 tests/case-overflow.test.js\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(
        filter
            .decide(
                Stream::Stdout,
                b"p0#test:  \xE2\x9C\x93 tests/case-after-overflow.test.js\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(!filter.can_compact());
}

#[test]
fn source_state_limit_is_shared_by_javascript_and_ecosystem_recognizers() {
    let mut plan = Plan::default();
    plan.families.extend([Family::Test, Family::RustTest]);
    plan.source_aliases = (0..=2048)
        .map(|index| (format!("p{index}"), format!("p{index}")))
        .collect();
    let mut filter = DispatchFilter::new(plan);
    for index in 0..3 {
        let record = format!("p0#test:  ✓ tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    for index in 1..2048 {
        let record = format!("p{index}#test:  ✓ tests/case-{index}.test.js\n");
        assert!(
            filter
                .decide(Stream::Stdout, record.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter
            .decide(
                Stream::Stdout,
                b"p0#test:  \xE2\x9C\x93 tests/case-3.test.js\n"
            )
            .unwrap(),
        Some(CompactKind::Passing)
    ));
    assert!(
        filter
            .decide(
                Stream::Stdout,
                b"p2048#test:  \xE2\x9C\x93 tests/case-overflow.test.js\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(
        filter
            .decide(
                Stream::Stdout,
                b"p0#test:  \xE2\x9C\x93 tests/case-after-overflow.test.js\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(!filter.can_compact());
}
