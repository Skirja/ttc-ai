#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, ManifestHints, classify};
use std::ffi::OsString;
use std::fs;

fn words(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

#[test]
fn direct_tools_and_nested_dispatch_map_to_families() {
    let dir = TestDir::new();
    for (args, family) in [
        (vec!["vitest", "run"], Family::Test),
        (vec!["npx", "jest"], Family::Test),
        (vec!["pnpm", "exec", "eslint", "."], Family::Lint),
        (vec!["bunx", "tsc", "--noEmit"], Family::Typecheck),
        (vec!["vite", "build"], Family::Build),
        (vec!["prettier", "--check", "."], Family::Format),
        (vec!["npm", "ci"], Family::Install),
    ] {
        let plan = classify(&words(&args), dir.path(), None);
        assert_eq!(plan.families, vec![family], "{args:?}");
    }
}

#[test]
fn all_specified_javascript_tools_have_a_family() {
    let dir = TestDir::new();
    let cases: &[(&[&str], Family)] = &[
        (&["vitest", "run"], Family::Test),
        (&["jest"], Family::Test),
        (&["playwright", "test"], Family::Test),
        (&["cypress", "run"], Family::Test),
        (&["mocha"], Family::Test),
        (&["ava"], Family::Test),
        (&["tap"], Family::Test),
        (&["eslint"], Family::Lint),
        (&["biome", "check"], Family::Lint),
        (&["biome", "lint"], Family::Lint),
        (&["oxlint"], Family::Lint),
        (&["stylelint"], Family::Lint),
        (&["tsc", "--noEmit"], Family::Typecheck),
        (&["vue-tsc"], Family::Typecheck),
        (&["svelte-check"], Family::Typecheck),
        (&["flow"], Family::Typecheck),
        (&["vite", "build"], Family::Build),
        (&["next", "build"], Family::Build),
        (&["nuxt", "build"], Family::Build),
        (&["webpack"], Family::Build),
        (&["rollup"], Family::Build),
        (&["esbuild"], Family::Build),
        (&["tsup"], Family::Build),
        (&["swc"], Family::Build),
        (&["prettier", "--check"], Family::Format),
        (&["biome", "format"], Family::Format),
        (&["dprint", "check"], Family::Format),
    ];
    for (args, family) in cases {
        let plan = classify(&words(args), dir.path(), None);
        assert_eq!(plan.families, vec![*family], "{args:?}");
    }
}

#[test]
fn package_managers_and_workspace_modifiers_supply_safe_candidates() {
    let dir = TestDir::new();
    fs::write(dir.path().join("package.json"), r#"{"scripts":{"test":"vitest run","lint":"eslint .","typecheck":"tsc --noEmit","build":"vite build"}}"#).unwrap();
    let cases: &[(&[&str], Family)] = &[
        (&["npm", "test"], Family::Test),
        (&["npm", "run", "lint"], Family::Lint),
        (&["pnpm", "typecheck"], Family::Typecheck),
        (&["yarn", "build"], Family::Build),
        (&["bun", "run", "test"], Family::Test),
        (&["npm", "--workspace", "api", "test"], Family::Test),
        (&["pnpm", "-r", "test"], Family::Test),
        (&["pnpm", "--filter", "api", "test"], Family::Test),
        (&["yarn", "workspace", "api", "test"], Family::Test),
        (
            &["yarn", "workspaces", "foreach", "run", "test"],
            Family::Test,
        ),
    ];
    for (args, family) in cases {
        let plan = classify(&words(args), dir.path(), None);
        assert_eq!(plan.families, vec![*family], "{args:?}");
    }
}

#[test]
fn scripts_are_read_statically_and_compound_check_combines_families() {
    let dir = TestDir::new();
    fs::write(dir.path().join("package.json"), r#"{"scripts":{"check":"npx tsc --noEmit && npm run lint","lint":"eslint .","cycle":"npm run cycle"}}"#).unwrap();
    let hints = ManifestHints::load(dir.path()).unwrap();
    let plan = classify(&words(&["npm run check"]), dir.path(), Some(&hints));
    assert_eq!(plan.families, vec![Family::Typecheck, Family::Lint]);
    assert!(
        classify(&words(&["npm", "run", "cycle"]), dir.path(), Some(&hints))
            .families
            .is_empty()
    );
}

#[test]
fn unknown_structured_or_ambiguous_command_stays_raw() {
    let dir = TestDir::new();
    for args in [
        vec!["custom", "test"],
        vec!["vitest", "--json"],
        vec!["vitest", "--jsonl"],
        vec!["vitest", "--xml"],
        vec!["vitest", "--yaml"],
        vec!["vitest", "--sarif"],
        vec!["vitest", "--output", "result.txt"],
        vec!["vitest", "-o", "result.txt"],
        vec!["vitest", "--format", "json"],
        vec!["vitest", "--reporter", "json"],
        vec!["vitest | cat"],
        vec!["vitest $(echo run)"],
    ] {
        let plan = classify(&words(&args), dir.path(), None);
        assert!(plan.raw || plan.families.is_empty(), "{args:?}");
    }
    fs::write(dir.path().join("package.json"), "{").unwrap();
    assert!(
        classify(&words(&["npm", "test"]), dir.path(), None)
            .families
            .is_empty()
    );
    assert!(
        classify(
            &words(&["npm", "--workspace", "api", "test"]),
            dir.path(),
            None
        )
        .raw
    );
}
