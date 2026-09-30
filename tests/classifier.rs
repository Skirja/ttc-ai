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
fn all_m8_commands_have_an_explicit_filter_family() {
    let dir = TestDir::new();
    let cases: &[(&[&str], Family)] = &[
        (&["cmake", "--build", "build"], Family::CmakeBuild),
        (&["ctest", "--test-dir", "build"], Family::Ctest),
        (&["ninja"], Family::NinjaBuild),
        (&["make", "test"], Family::MakeBuild),
        (&["make", "check"], Family::MakeBuild),
        (&["rspec"], Family::Rspec),
        (&["bundle", "exec", "rspec"], Family::Rspec),
        (&["rubocop"], Family::Rubocop),
        (&["rake", "test"], Family::RakeTest),
        (&["swift", "build"], Family::SwiftBuild),
        (&["swift", "test"], Family::SwiftTest),
        (&["docker", "build", "."], Family::ContainerBuild),
        (&["docker", "compose", "build"], Family::ContainerBuild),
        (&["podman", "build", "."], Family::ContainerBuild),
        (&["terraform", "validate"], Family::TerraformValidate),
        (&["helm", "lint", "charts/demo"], Family::HelmLint),
    ];
    for (args, family) in cases {
        let plan = classify(&words(args), dir.path(), None);
        assert_eq!(plan.families, vec![*family], "{args:?}");
    }
}

#[test]
fn m8_dynamic_targets_machine_output_and_mutating_ruby_commands_stay_raw() {
    let dir = TestDir::new();
    for args in [
        vec!["make", "all"],
        vec!["make", "test", "release"],
        vec!["make", "-f", "custom.mk", "test"],
        vec!["cmake", "--build", "build", "--target", "all"],
        vec!["cmake", "--build", "build", "--target=install"],
        vec!["ninja", "custom-target"],
        vec![
            "ctest",
            "--test-dir",
            "build",
            "--output-junit",
            "results.xml",
        ],
        vec!["ctest", "--test-dir", "build", "-T", "Test"],
        vec!["rspec", "--format", "json"],
        vec!["rubocop", "--autocorrect"],
        vec!["rake", "test", "--trace"],
        vec!["docker", "build", "--progress=json", "."],
        vec!["podman", "build", "--quiet", "."],
        vec!["terraform", "validate", "-json"],
        vec!["helm", "lint", "charts/demo", "--output", "json"],
        vec!["swift", "test", "--dump-tests-json"],
    ] {
        let plan = classify(&words(&args), dir.path(), None);
        assert!(plan.raw || plan.families.is_empty(), "{args:?}: {plan:?}");
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
        (&["uv", "run", "--with", "pytest", "pytest"], Family::PyTest),
        (&["uv", "run", "--project=app", "pytest"], Family::PyTest),
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
            &["coverage", "run", "--branch", "-m", "pytest"],
            Family::PyTest,
        ),
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
        vec!["uv", "run", "--with", "pytest", "python", "app.py"],
        vec!["uv", "run", "--project", "pytest", "python", "app.py"],
        vec!["uv", "run", "--unknown", "pytest"],
        vec!["coverage", "run", "app.py", "-m", "pytest"],
        vec!["python", "-m", "coverage", "run", "app.py", "-m", "pytest"],
        vec!["go", "test", "-json", "&&", "echo", "done"],
        vec!["go", "test", "--", "-json"],
        vec!["go", "test", "-json=false"],
        vec!["go", "test", "--json"],
    ] {
        let plan = classify(&words(&args), dir.path(), None);
        assert!(plan.raw || plan.families.is_empty(), "{args:?}: {plan:?}");
    }
}

#[test]
fn php_jvm_and_dotnet_commands_select_only_their_specific_families() {
    let dir = TestDir::new();
    let cases: &[(&[&str], &[Family])] = &[
        (&["phpunit"], &[Family::PhpTest]),
        (&["vendor/bin/phpunit"], &[Family::PhpTest]),
        (&["pest"], &[Family::PhpTest]),
        (&["vendor/bin/pest"], &[Family::PhpTest]),
        (&["php", "artisan", "test"], &[Family::PhpTest]),
        (
            &["php", "vendor/bin/phpunit", "--testdox"],
            &[Family::PhpTest],
        ),
        (
            &["php", "-d", "memory_limit=-1", "vendor/bin/pest"],
            &[Family::PhpTest],
        ),
        (&["phpstan", "analyse"], &[Family::PhpLint]),
        (&["psalm"], &[Family::PhpTypecheck]),
        (&["phpcs"], &[Family::PhpLint]),
        (&["php-cs-fixer", "fix", "--dry-run"], &[Family::PhpFormat]),
        (
            &["mvn", "test"],
            &[Family::JvmBuild, Family::JvmProgress, Family::JvmTest],
        ),
        (
            &["./mvnw", "verify", "-B"],
            &[Family::JvmBuild, Family::JvmProgress, Family::JvmTest],
        ),
        (
            &["gradle", "test"],
            &[Family::JvmBuild, Family::JvmProgress, Family::JvmTest],
        ),
        (
            &["./gradlew", "build"],
            &[Family::JvmBuild, Family::JvmProgress, Family::JvmTest],
        ),
        (&["javac", "Main.java"], &[Family::JvmCompile]),
        (
            &[
                "java",
                "-jar",
                "junit-platform-console-standalone.jar",
                "execute",
            ],
            &[Family::JvmTest],
        ),
        (
            &["dotnet", "test"],
            &[
                Family::DotnetTest,
                Family::DotnetBuild,
                Family::DotnetRestore,
            ],
        ),
        (
            &["dotnet", "test", "sample.sln"],
            &[
                Family::DotnetTest,
                Family::DotnetBuild,
                Family::DotnetRestore,
            ],
        ),
        (
            &["dotnet", "build"],
            &[Family::DotnetBuild, Family::DotnetRestore],
        ),
        (&["dotnet", "restore"], &[Family::DotnetRestore]),
        (
            &["dotnet", "publish"],
            &[Family::DotnetBuild, Family::DotnetRestore],
        ),
        (
            &["dotnet", "format", "--verify-no-changes"],
            &[Family::DotnetFormat],
        ),
    ];
    for (args, families) in cases {
        let plan = classify(&words(args), dir.path(), None);
        assert_eq!(&plan.families, families, "{args:?}: {plan:?}");
        assert!(!plan.raw, "{args:?}: {plan:?}");
    }
}

#[test]
fn php_applications_and_unrecognized_reporters_remain_raw() {
    let dir = TestDir::new();
    for args in [
        vec!["php", "app.php"],
        vec!["php", "artisan", "serve"],
        vec!["phpunit", "--log-junit", "results.xml"],
        vec!["phpunit", "--reporter", "verbose"],
        vec!["pest", "--compact"],
        vec!["phpcs", "--report=json"],
        vec!["mvn", "-X", "test"],
        vec!["gradle", "test", "--console=rich"],
        vec![
            "java",
            "-jar",
            "junit-platform-console-standalone.jar",
            "--details=flat",
        ],
        vec!["dotnet", "test", "--logger", "trx"],
        vec!["dotnet", "test", "--logger:trx"],
        vec!["dotnet", "format", "."],
        vec!["vitest", "--reporter", "custom"],
    ] {
        let plan = classify(&words(&args), dir.path(), None);
        assert!(plan.raw || plan.families.is_empty(), "{args:?}: {plan:?}");
    }
}

#[test]
fn composer_scripts_resolve_static_aliases_and_fail_closed_on_cycles() {
    let dir = TestDir::new();
    fs::write(
        dir.path().join("composer.json"),
        r#"{"scripts":{"test":"@php vendor/bin/phpunit --testdox tests/CalculatorTest.php","check":["@test"],"analyse":"@php vendor/bin/phpstan analyse --no-progress","cycle":"@cycle","dynamic":"php app.php"}}"#,
    ).unwrap();
    let hints = ManifestHints::load(dir.path()).unwrap();
    let test_plan = classify(&words(&["composer", "test"]), dir.path(), Some(&hints));
    assert_eq!(test_plan.families, vec![Family::PhpTest]);
    assert!(!test_plan.raw, "{test_plan:?}");
    assert_eq!(
        classify(
            &words(&["composer", "run-script", "check"]),
            dir.path(),
            Some(&hints)
        )
        .families,
        vec![Family::PhpTest]
    );
    assert_eq!(
        classify(&words(&["composer", "analyse"]), dir.path(), Some(&hints)).families,
        vec![Family::PhpLint]
    );
    assert!(classify(&words(&["composer", "cycle"]), dir.path(), Some(&hints)).raw);
    assert!(
        classify(&words(&["composer", "dynamic"]), dir.path(), Some(&hints))
            .families
            .is_empty()
    );
}

#[test]
fn composer_lifecycle_callbacks_and_dynamic_build_targets_stay_raw() {
    let composer = TestDir::new();
    fs::write(
        composer.path().join("composer.json"),
        r#"{"scripts":{"post-install-cmd":"php scripts/setup.php","test":"@php vendor/bin/phpunit"}}"#,
    )
    .unwrap();
    let hints = ManifestHints::load(composer.path()).unwrap();
    assert!(
        classify(
            &words(&["composer", "install"]),
            composer.path(),
            Some(&hints)
        )
        .raw
    );

    let maven = TestDir::new();
    fs::write(
        maven.path().join("pom.xml"),
        "<project><modules><module>api</module></modules></project>",
    )
    .unwrap();
    let hints = ManifestHints::load(maven.path()).unwrap();
    assert!(
        !classify(
            &words(&["mvn", "-pl", "api", "test"]),
            maven.path(),
            Some(&hints),
        )
        .raw
    );
    assert!(
        classify(
            &words(&["mvn", "-pl", "unknown", "test"]),
            maven.path(),
            Some(&hints),
        )
        .raw
    );

    let gradle = TestDir::new();
    fs::write(gradle.path().join("settings.gradle"), "include ':api'\n").unwrap();
    let hints = ManifestHints::load(gradle.path()).unwrap();
    assert!(
        !classify(
            &words(&["gradle", ":api:test"]),
            gradle.path(),
            Some(&hints),
        )
        .raw
    );
    fs::write(
        gradle.path().join("settings.gradle"),
        "if (providers.gradleProperty(\"ci\").isPresent) include(\":api\")\n",
    )
    .unwrap();
    let hints = ManifestHints::load(gradle.path()).unwrap();
    assert!(classify(&words(&["gradle", "test"]), gradle.path(), Some(&hints)).raw);
}
