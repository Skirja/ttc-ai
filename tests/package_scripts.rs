#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, ManifestHints, classify};
use std::ffi::OsString;
use std::fs;

fn words(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn nested_compound_scripts_combine_ecosystems_and_honor_workspace_cwd() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::write(dir.path().join("package.json"), r#"{"name":"root","workspaces":["packages/*"],"scripts":{"check":"npm run lint && go test ./...","lint":"npx eslint ."}}"#).unwrap();
    fs::write(
        dir.path().join("packages/api/package.json"),
        r#"{"name":"api","scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let plan = classify(&words(&["npm run check"]), dir.path(), Some(&hints));
    assert_eq!(plan.families, vec![Family::Lint, Family::GoTest]);
    assert!(!plan.raw);

    let plan = classify(
        &words(&["cd packages/api && pnpm test"]),
        dir.path(),
        Some(&hints),
    );
    assert_eq!(plan.families, vec![Family::Test]);
    assert!(!plan.raw);
    assert_eq!(plan.sources, vec!["api"]);
}

#[test]
fn discovery_is_reused_within_one_workspace_and_other_roots_fail_raw() {
    let dir = TestDir::new();
    for project in ["a", "b"] {
        fs::create_dir_all(dir.path().join(project)).unwrap();
        fs::write(
            dir.path().join(project).join("package.json"),
            r#"{"scripts":{"test":"vitest run"}}"#,
        )
        .unwrap();
    }
    let one = classify(&words(&["cd a && npm run test"]), dir.path(), None);
    assert_eq!(one.families, vec![Family::Test]);
    assert!(!one.raw);
    let repeated = classify(
        &words(&["cd a && npm run test; npm run test"]),
        dir.path(),
        None,
    );
    assert_eq!(repeated.families, vec![Family::Test]);
    assert!(!repeated.raw);
    let different_root = classify(
        &words(&["cd a && npm run test; cd ../b && npm run test"]),
        dir.path(),
        None,
    );
    assert!(different_root.raw);
    for command in [
        "cd - && npm run test",
        "cd && npm run test",
        "cd missing && npm run test",
    ] {
        assert!(classify(&words(&[command]), dir.path(), None).raw);
    }
}

#[test]
fn cycle_depth_overflow_unknown_app_and_structured_script_fail_open() {
    let dir = TestDir::new();
    let mut json_scripts = String::from(r#""#);
    for index in 0..17 {
        let next = if index == 16 {
            "vitest run".to_owned()
        } else {
            format!("npm run s{}", index + 1)
        };
        if !json_scripts.is_empty() {
            json_scripts.push(',');
        }
        json_scripts.push_str(&format!("\"s{index}\":\"{next}\""));
    }
    json_scripts.push_str(
        r#","cycle":"npm run cycle","app":"node app.js","structured":"vitest run --reporter=json""#,
    );
    fs::write(
        dir.path().join("package.json"),
        format!("{{\"scripts\":{{{json_scripts}}}}}"),
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let too_deep = classify(&words(&["npm run s0"]), dir.path(), Some(&hints));
    assert!(too_deep.raw);
    let depth_16 = classify(&words(&["npm run s1"]), dir.path(), Some(&hints));
    assert!(depth_16.families.contains(&Family::Test));
    assert!(!depth_16.raw);
    assert!(classify(&words(&["npm run cycle"]), dir.path(), Some(&hints)).raw);
    assert!(classify(&words(&["npm run app"]), dir.path(), Some(&hints)).raw);
    assert!(classify(&words(&["npm run structured"]), dir.path(), Some(&hints)).raw);

    let plan = classify(
        &[OsString::from("npm run lint && go test ./...")],
        dir.path(),
        Some(&hints),
    );
    assert!(plan.families.contains(&Family::GoTest));
    assert!(plan.raw);
}

#[test]
fn nested_alias_cycles_are_scoped_to_active_manager_project_and_script_frames() {
    let dir = TestDir::new();
    fs::create_dir_all(dir.path().join("packages/api")).unwrap();
    fs::create_dir_all(dir.path().join("packages/web")).unwrap();
    fs::write(
        dir.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"],"scripts":{"a":"npm run b","b":"npm run a","twice":"npm run helper && npm run helper","helper":"vitest run"}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/api/package.json"),
        r#"{"name":"@m6/api","scripts":{"cycle":"npm run cycle --workspace @m6/web","test":"npm run helper --workspace @m6/web"}}"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("packages/web/package.json"),
        r#"{"name":"@m6/web","scripts":{"cycle":"npm run cycle --workspace @m6/api","helper":"vitest run"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();

    let indirect = classify(&words(&["npm run a"]), dir.path(), Some(&hints));
    assert!(indirect.raw);
    let cross_project = classify(
        &words(&["npm run cycle --workspace @m6/api"]),
        dir.path(),
        Some(&hints),
    );
    assert!(cross_project.raw);
    let shared_alias = classify(&words(&["npm run twice"]), dir.path(), Some(&hints));
    assert!(shared_alias.families.contains(&Family::Test));
    assert!(!shared_alias.raw);
    let other_project = classify(
        &words(&["npm run test --workspace @m6/api"]),
        dir.path(),
        Some(&hints),
    );
    assert!(other_project.families.contains(&Family::Test));
    assert!(!other_project.raw);
}

#[test]
fn quoted_operator_is_an_argument_but_pipeline_and_background_are_not_classified() {
    let dir = TestDir::new();
    fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"test":"vitest run"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::discover(dir.path()).unwrap().unwrap();
    let quoted = classify(
        &[OsString::from("npm run test -- --name 'a && b'")],
        dir.path(),
        Some(&hints),
    );
    assert!(quoted.families.contains(&Family::Test));
    let pipeline = classify(
        &[OsString::from("npm test | cat")],
        dir.path(),
        Some(&hints),
    );
    assert!(pipeline.raw);
}

#[test]
fn lifecycle_resolution_follows_package_manager_metadata_and_configuration() {
    let yarn = TestDir::new();
    fs::write(yarn.path().join("package.json"), r#"{"name":"root","packageManager":"yarn@4.18.1","scripts":{"pretest":"cargo test","test":"vitest run"}}"#).unwrap();
    let hints = ManifestHints::discover(yarn.path()).unwrap().unwrap();
    let modern = classify(&words(&["yarn run test"]), yarn.path(), Some(&hints));
    assert_eq!(modern.families, vec![Family::Test]);

    fs::write(yarn.path().join("package.json"), r#"{"name":"root","packageManager":"yarn@1.22.22","scripts":{"pretest":"cargo test","test":"vitest run"}}"#).unwrap();
    let hints = ManifestHints::discover(yarn.path()).unwrap().unwrap();
    let classic = classify(&words(&["yarn run test"]), yarn.path(), Some(&hints));
    assert!(classic.families.contains(&Family::RustTest));
    assert!(classic.families.contains(&Family::Test));

    let pnpm = TestDir::new();
    fs::write(
        pnpm.path().join("package.json"),
        r#"{"name":"root","scripts":{"pretest":"cargo test","test":"vitest run"}}"#,
    )
    .unwrap();
    fs::write(
        pnpm.path().join("pnpm-workspace.yaml"),
        "enablePrePostScripts: true\n",
    )
    .unwrap();
    let hints = ManifestHints::discover(pnpm.path()).unwrap().unwrap();
    let enabled = classify(&words(&["pnpm run test"]), pnpm.path(), Some(&hints));
    assert!(enabled.families.contains(&Family::RustTest));
    assert!(enabled.families.contains(&Family::Test));
    let suppressed = classify(
        &words(&["pnpm run test --ignore-scripts"]),
        pnpm.path(),
        Some(&hints),
    );
    assert_eq!(suppressed.families, vec![Family::Test]);
    assert!(!suppressed.raw);
}
