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
    ];
    for (args, family) in cases {
        let plan = classify(&words(args), dir.path(), None);
        assert_eq!(plan.families, vec![*family], "{args:?}");
    }
    for args in [
        vec!["npm", "--workspace", "api", "test"],
        vec!["npm", "--workspace=api", "test"],
        vec!["npm", "-wapi", "test"],
        vec!["npm", "run", "test", "--workspaces"],
        vec!["pnpm", "-r", "test"],
        vec!["pnpm", "--filter", "api", "test"],
        vec!["pnpm", "--filter=api", "test"],
        vec!["yarn", "workspace", "api", "test"],
        vec!["yarn", "workspaces", "foreach", "run", "test"],
    ] {
        assert!(classify(&words(&args), dir.path(), None).raw, "{args:?}");
    }
}

#[test]
fn scripts_are_read_statically_and_compound_check_combines_families() {
    let dir = TestDir::new();
    fs::write(dir.path().join("package.json"), r#"{"scripts":{"check":"npx tsc --noEmit && npm run lint","lint":"eslint .","cycle":"npm run cycle","structured":"vitest run --reporter=verbose && vitest run --reporter=json","nested":"npm run structured"}}"#).unwrap();
    let hints = ManifestHints::load(dir.path()).unwrap();
    let plan = classify(&words(&["npm run check"]), dir.path(), Some(&hints));
    assert_eq!(plan.families, vec![Family::Typecheck, Family::Lint]);
    assert!(
        classify(&words(&["npm", "run", "cycle"]), dir.path(), Some(&hints))
            .families
            .is_empty()
    );
    assert!(
        classify(
            &words(&["npm", "run", "structured"]),
            dir.path(),
            Some(&hints)
        )
        .raw
    );
    assert!(classify(&words(&["npm", "run", "nested"]), dir.path(), Some(&hints)).raw);
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
        vec!["cargo", "test", "--message-format=json"],
        vec![
            "cargo",
            "test",
            "--message-format",
            "json-render-diagnostics",
        ],
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

#[test]
fn rust_python_and_go_commands_select_tool_specific_parsers() {
    let dir = TestDir::new();
    let cases: &[(&[&str], Family)] = &[
        (&["cargo", "test"], Family::RustTest),
        (&["cargo", "test", "--workspace"], Family::RustTest),
        (&["cargo", "test", "-p", "api"], Family::RustTest),
        (&["cargo", "nextest", "run"], Family::RustNextest),
        (&["cargo", "build", "--workspace"], Family::RustBuild),
        (&["cargo", "check"], Family::RustCheck),
        (&["cargo", "clippy"], Family::RustClippy),
        (&["cargo", "fmt", "--check"], Family::RustFmt),
        (&["cargo", "doc"], Family::RustDoc),
        (&["python", "-m", "pytest"], Family::PyTest),
        (&["python3.12", "-m", "unittest"], Family::PyUnittest),
        (&["pytest"], Family::PyTest),
        (&["uv", "run", "pytest"], Family::PyTest),
        (&["poetry", "run", "ruff", "check"], Family::PyRuff),
        (&["pipenv", "run", "mypy"], Family::PyMypy),
        (&["tox"], Family::PyTox),
        (&["nox"], Family::PyNox),
        (&["ruff", "check"], Family::PyRuff),
        (&["pyright"], Family::PyPyright),
        (&["pylint"], Family::PyPylint),
        (&["black", "--check", "."], Family::PyBlack),
        (&["coverage", "report"], Family::PyCoverage),
        (&["coverage", "run", "-m", "pytest"], Family::PyTest),
        (
            &["python", "-m", "coverage", "run", "-m", "pytest"],
            Family::PyTest,
        ),
        (&["pip", "install", "sample"], Family::PyInstall),
        (&["uv", "pip", "install", "sample"], Family::PyInstall),
        (&["poetry", "install"], Family::PyInstall),
        (&["pipenv", "install"], Family::PyInstall),
        (&["go", "test", "./..."], Family::GoTest),
        (&["go", "build", "./..."], Family::GoBuild),
        (&["go", "vet", "./..."], Family::GoVet),
        (&["go", "generate", "./..."], Family::GoGenerate),
        (&["golangci-lint", "run"], Family::GoLint),
        (&["staticcheck", "./..."], Family::GoStaticcheck),
    ];
    for (args, family) in cases {
        let plan = classify(&words(args), dir.path(), None);
        assert_eq!(plan.families, vec![*family], "{args:?}");
        assert!(!plan.raw, "{args:?}");
    }
    for args in [
        vec!["go", "test", "-json"],
        vec!["go", "-C", "subdir", "test", "-json", "./..."],
    ] {
        assert_eq!(
            classify(&words(&args), dir.path(), None).families,
            vec![Family::GoJson]
        );
    }
}

#[test]
fn unsupported_generic_apps_and_compound_go_json_stay_raw() {
    let dir = TestDir::new();
    for args in [
        vec!["python", "app.py"],
        vec!["go", "test", "-json", "&&", "echo", "done"],
        vec!["go", "test", "--", "-json"],
        vec!["go", "test", "-json=false"],
        vec!["go", "test", "--json"],
    ] {
        let plan = classify(&words(&args), dir.path(), None);
        assert!(plan.raw || plan.families.is_empty(), "{args:?}: {plan:?}");
    }
}
