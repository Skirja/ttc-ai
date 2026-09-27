#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::manifests::WorkspaceHints;
use std::fs;
use std::os::unix::fs::symlink;

#[test]
fn discovers_pnpm_workspace_from_nested_package_and_resolves_dependency_selectors() {
    let dir = TestDir::new();
    let root = dir.path();
    fs::create_dir_all(root.join("packages/api")).unwrap();
    fs::create_dir_all(root.join("packages/shared")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"root","packageManager":"pnpm@9.15.9","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - 'packages/*'\n",
    )
    .unwrap();
    fs::write(root.join("packages/api/package.json"), r#"{"name":"@demo/api","scripts":{"test":"vitest run"},"dependencies":{"@demo/shared":"workspace:*"}}"#).unwrap();
    fs::write(
        root.join("packages/shared/package.json"),
        r#"{"name":"@demo/shared","scripts":{"test":"jest"}}"#,
    )
    .unwrap();

    let hints = WorkspaceHints::discover(&root.join("packages/api"))
        .unwrap()
        .unwrap();
    assert_eq!(hints.root, root);
    assert_eq!(hints.projects.len(), 3);
    let selected = hints.select_packages(&["@demo/api...".into()]);
    let names: Vec<_> = selected
        .iter()
        .filter_map(|project| project.name.as_deref())
        .collect();
    assert!(names.contains(&"@demo/api"));
    assert!(names.contains(&"@demo/shared"));
}

#[test]
fn malformed_duplicate_json_and_yaml_fail_discovery() {
    let dir = TestDir::new();
    fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"test":"vitest"},"scripts":{"test":"jest"}}"#,
    )
    .unwrap();
    assert!(WorkspaceHints::discover(dir.path()).is_err());

    let pnpm = TestDir::new();
    fs::write(pnpm.path().join("package.json"), r#"{"name":"root"}"#).unwrap();
    fs::write(
        pnpm.path().join("pnpm-workspace.yaml"),
        "packages:\n  - packages/*\npackages:\n  - apps/*\n",
    )
    .unwrap();
    assert!(WorkspaceHints::discover(pnpm.path()).is_err());

    for source in [
        "packages: &workspace\n  - packages/*\n",
        "packages: !!str packages/*\n",
        "packages:\n  - *missing\n",
    ] {
        let unsupported = TestDir::new();
        fs::write(
            unsupported.path().join("package.json"),
            r#"{"name":"root"}"#,
        )
        .unwrap();
        fs::write(unsupported.path().join("pnpm-workspace.yaml"), source).unwrap();
        assert!(WorkspaceHints::discover(unsupported.path()).is_err());
    }
}

#[test]
fn cargo_member_globs_and_go_work_use_paths_are_discovered_without_execution() {
    let dir = TestDir::new();
    let root = dir.path();
    fs::create_dir_all(root.join("crates/alpha")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/alpha/Cargo.toml"),
        "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("go.work"),
        "go 1.27.1\nuse (\n ./crates/alpha\n)\n",
    )
    .unwrap();
    fs::write(
        root.join("crates/alpha/go.mod"),
        "module example.com/alpha\ngo 1.27.1\n",
    )
    .unwrap();
    let hints = WorkspaceHints::discover(root).unwrap().unwrap();
    assert!(hints.has_cargo_workspace);
    assert!(hints.has_go_workspace);
    assert!(
        hints
            .projects
            .iter()
            .any(|project| project.name.as_deref() == Some("alpha"))
    );
}

#[test]
fn workspace_exclusions_and_symlink_traversal_do_not_add_unselected_projects() {
    let dir = TestDir::new();
    let root = dir.path();
    fs::create_dir_all(root.join("packages/api")).unwrap();
    fs::create_dir_all(root.join("packages/ignored")).unwrap();
    fs::write(
        root.join("package.json"),
        r#"{"name":"root","workspaces":["packages/*","!packages/ignored"]}"#,
    )
    .unwrap();
    fs::write(root.join("packages/api/package.json"), r#"{"name":"api"}"#).unwrap();
    fs::write(
        root.join("packages/ignored/package.json"),
        r#"{"name":"ignored"}"#,
    )
    .unwrap();
    symlink(root, root.join("packages/loop")).unwrap();
    let external = TestDir::new();
    fs::write(
        external.path().join("package.json"),
        r#"{"name":"external"}"#,
    )
    .unwrap();
    symlink(external.path(), root.join("packages/external")).unwrap();

    let hints = WorkspaceHints::discover(root).unwrap().unwrap();
    let names: Vec<_> = hints
        .projects
        .iter()
        .filter_map(|project| project.name.as_deref())
        .collect();
    assert!(names.contains(&"api"));
    assert!(!names.contains(&"ignored"));
    assert!(!names.contains(&"external"));
}

#[test]
fn manifest_and_invocation_resource_limits_fail_discovery_closed() {
    let exact = TestDir::new();
    let mut exact_package = br#"{"name":"exact"}"#.to_vec();
    exact_package.resize(1024 * 1024, b' ');
    fs::write(exact.path().join("package.json"), exact_package).unwrap();
    assert!(WorkspaceHints::discover(exact.path()).is_ok());

    let oversized = TestDir::new();
    let mut package = br#"{"name":"large"}"#.to_vec();
    package.resize(1024 * 1024 + 1, b' ');
    fs::write(oversized.path().join("package.json"), package).unwrap();
    assert!(WorkspaceHints::discover(oversized.path()).is_err());

    let aggregate = TestDir::new();
    fs::create_dir_all(aggregate.path().join("packages")).unwrap();
    fs::write(
        aggregate.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    let mut child = br#"{"name":"child"}"#.to_vec();
    child.resize(1024 * 1024 - 1, b' ');
    for index in 0..17 {
        let directory = aggregate.path().join(format!("packages/p{index}"));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("package.json"), &child).unwrap();
    }
    assert!(WorkspaceHints::discover(aggregate.path()).is_err());
}

#[test]
fn project_entry_and_ancestor_limits_are_enforced() {
    let projects = TestDir::new();
    fs::create_dir_all(projects.path().join("packages")).unwrap();
    fs::write(
        projects.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    for index in 0..4097 {
        let directory = projects.path().join(format!("packages/p{index}"));
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join("package.json"),
            format!(r#"{{"name":"p{index}"}}"#),
        )
        .unwrap();
    }
    assert!(WorkspaceHints::discover(projects.path()).is_err());

    let entries = TestDir::new();
    fs::create_dir_all(entries.path().join("packages")).unwrap();
    fs::write(
        entries.path().join("package.json"),
        r#"{"name":"root","workspaces":["packages/*"]}"#,
    )
    .unwrap();
    for index in 0..16_385 {
        fs::write(
            entries
                .path()
                .join("packages")
                .join(format!("entry-{index}")),
            [],
        )
        .unwrap();
    }
    assert!(WorkspaceHints::discover(entries.path()).is_err());

    let ancestors = TestDir::new();
    fs::write(
        ancestors.path().join("package.json"),
        r#"{"name":"root","workspaces":[]}"#,
    )
    .unwrap();
    let mut accepted = ancestors.path().to_path_buf();
    for _ in 0..63 {
        accepted.push("d");
    }
    fs::create_dir_all(&accepted).unwrap();
    assert!(WorkspaceHints::discover(&accepted).is_ok());
    let mut too_deep = ancestors.path().to_path_buf();
    for _ in 0..64 {
        too_deep.push("d");
    }
    fs::create_dir_all(&too_deep).unwrap();
    assert!(WorkspaceHints::discover(&too_deep).is_err());
}
