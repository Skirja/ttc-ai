use serde_json::json;
use std::path::Path;
use tempfile::tempdir;
use ttc_ai::{
    command::*,
    config::Config,
    filtering::Filter,
    integrations,
    recovery::{Record, Store},
    resolver::classify,
};
fn package(dir: &Path, scripts: serde_json::Value) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("package.json"),
        json!({"name":dir.file_name().and_then(|s|s.to_str()).unwrap_or("root"),"scripts":scripts})
            .to_string(),
    )
    .unwrap();
}
#[test]
fn parser_preserves_quotes_and_spans() {
    let s = "pnpm run test -- 'a b' \"x'y\" && cargo test";
    let a = parse(s, Shell::Posix);
    assert!(!a.uncertain, "{a:?}");
    assert_eq!(a.commands.len(), 2);
    assert_eq!(a.commands[0].argv[4], "a b");
    assert_eq!(&s[a.commands[1].start..a.commands[1].end], "cargo test");
}
#[test]
fn unsupported_expansions_fail_open() {
    for s in [
        "pnpm $(echo test)",
        "pnpm \"$TASK\"",
        "if true; then cargo test; fi",
        "cargo test &",
    ] {
        assert!(parse(s, Shell::Posix).uncertain, "{s}");
    }
}
#[test]
fn redirects_and_pipelines_are_exact() {
    for s in [
        "cargo test > result.txt",
        "go test ./... | tee results.log",
        "git diff > patch.diff",
        "curl URL | jq .",
        "pnpm test 2> errors.txt",
    ] {
        assert!(parse(s, Shell::Posix).exact_output, "{s}");
        assert!(!classify(s, Path::new("."), Shell::Posix).filterable);
    }
}
#[test]
fn windows_is_not_posix() {
    assert!(parse("cargo test; rm x", Shell::PowerShell).uncertain);
    assert!(parse("echo %PATH%", Shell::Cmd).uncertain);
    assert_eq!(
        parse("cargo test", Shell::Cmd).commands[0].argv,
        vec!["cargo", "test"]
    );
}
#[test]
fn script_identity_not_tool_identity() {
    let d = tempdir().unwrap();
    for (body, fam) in [
        ("biome check .", "js-lint"),
        ("prettier --check .", "js-lint"),
        ("cargo test", "cargo-test"),
    ] {
        package(d.path(), json!({"lint":body}));
        for s in ["pnpm lint", "npm run lint", "yarn lint", "bun run lint"] {
            let c = classify(s, d.path(), Shell::Posix);
            assert!(c.filterable, "{c:?}");
            assert_eq!(c.families, vec![fam]);
            assert!(serde_json::to_string(&c.workload).unwrap().contains(body));
        }
    }
}
#[test]
fn nested_composite_all_families() {
    let d = tempdir().unwrap();
    package(
        d.path(),
        json!({"check":"pnpm lint && pnpm typecheck && cargo clippy && go vet ./...","lint":"biome check .","typecheck":"tsc --noEmit"}),
    );
    let c = classify("bun run check", d.path(), Shell::Posix);
    assert!(c.filterable, "{c:?}");
    assert_eq!(c.families, vec!["go", "js-lint", "rust", "typescript"]);
}
#[test]
fn lifecycle_included_in_risk() {
    let d = tempdir().unwrap();
    package(
        d.path(),
        json!({"test":"vitest run","pretest":"rm -rf dist"}),
    );
    let c = classify("npm test", d.path(), Shell::Posix);
    assert_eq!(c.risk, Risk::Destructive);
    assert!(!c.filterable);
}
#[test]
fn flags_never_discarded() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"lint":"eslint . --max-warnings 0"}));
    let c = classify("npm run lint -- --fix", d.path(), Shell::Posix);
    assert_eq!(c.ast.source, "npm run lint -- --fix");
    assert!(
        serde_json::to_string(&c.workload)
            .unwrap()
            .contains("--max-warnings 0")
    );
}
#[test]
fn bun_builtin_is_distinct() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"test":"cargo test"}));
    assert_eq!(
        classify("bun test", d.path(), Shell::Posix).families,
        vec!["bun-test"]
    );
    assert_eq!(
        classify("bun run test", d.path(), Shell::Posix).families,
        vec!["cargo-test"]
    );
}
#[test]
fn package_exec_variants() {
    for s in [
        "pnpm exec vitest",
        "pnpm vitest",
        "pnpx vitest",
        "pnpm dlx vitest",
        "npx vitest",
        "bunx vitest",
        "bun x vitest",
    ] {
        assert_eq!(
            classify(s, Path::new("/tmp"), Shell::Posix).families,
            vec!["js-test"],
            "{s}"
        );
    }
}
#[test]
fn cycle_is_bounded() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"test":"pnpm check","check":"pnpm test"}));
    let c = classify("pnpm test", d.path(), Shell::Posix);
    assert!(!c.filterable);
    assert!(serde_json::to_string(&c).unwrap().contains("cycle"));
}
#[test]
fn manifest_cache_invalidates() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"test":"cargo test"}));
    assert_eq!(
        classify("pnpm test", d.path(), Shell::Posix).families,
        vec!["cargo-test"]
    );
    package(d.path(), json!({"test":"pytest"}));
    assert_eq!(
        classify("pnpm test", d.path(), Shell::Posix).families,
        vec!["pytest"]
    );
}
#[test]
fn monorepo_selects_correct_package() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"test":"pytest"}));
    std::fs::write(
        d.path().join("pnpm-workspace.yaml"),
        "packages:\n  - packages/*\n",
    )
    .unwrap();
    package(&d.path().join("packages/api"), json!({"test":"cargo test"}));
    package(&d.path().join("packages/web"), json!({"test":"vitest run"}));
    std::fs::write(
        d.path().join("packages/web/package.json"),
        json!({"name":"web","scripts":{"test":"vitest run"}}).to_string(),
    )
    .unwrap();
    for s in [
        "pnpm --filter api test",
        "yarn workspace api test",
        "nx run api:test",
    ] {
        let c = classify(s, d.path(), Shell::Posix);
        assert_eq!(c.families, vec!["cargo-test"], "{s}: {c:?}");
    }
}
#[test]
fn recursive_workspaces() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"test":"pytest"}));
    std::fs::write(
        d.path().join("pnpm-workspace.yaml"),
        "packages: [packages/*]",
    )
    .unwrap();
    package(&d.path().join("packages/api"), json!({"test":"cargo test"}));
    let c = classify("pnpm -r test", d.path(), Shell::Posix);
    assert!(c.families.contains(&"cargo-test".into()));
    assert!(c.families.contains(&"pytest".into()));
}
#[test]
fn container_context_is_unknown() {
    for s in [
        "docker compose exec api pnpm check",
        "docker compose run --rm worker cargo test",
    ] {
        let c = classify(s, Path::new("."), Shell::Posix);
        assert!(!c.filterable);
        assert!(
            serde_json::to_string(&c)
                .unwrap()
                .contains("container filesystem")
        );
    }
}
#[test]
fn python_wrappers() {
    for s in [
        "uv run pytest",
        "uv run python -m pytest",
        "poetry run pytest",
        "python3 -m pytest",
        ".venv/bin/pytest",
    ] {
        let c = classify(s, Path::new("."), Shell::Posix);
        assert_eq!(c.families, vec!["pytest"], "{s}");
    }
}
#[test]
fn explicit_structured_output_is_exact() {
    for s in [
        "git status --porcelain",
        "cargo test --json",
        "kubectl get pods -o json",
        "gh pr list --json number",
        "git log --oneline",
    ] {
        assert!(!classify(s, Path::new("."), Shell::Posix).filterable, "{s}");
    }
}
fn filtered(families: &[&str], s: &[u8]) -> Vec<u8> {
    let mut f = Filter::new(families.iter().map(|s| s.to_string()).collect());
    let mut b = vec![];
    for chunk in s.chunks(16384) {
        for e in f.feed(OutputEvent {
            channel: Channel::Stdout,
            bytes: chunk.to_vec(),
        }) {
            b.extend(e.bytes);
        }
    }
    for e in f.finish() {
        b.extend(e.bytes);
    }
    b
}
#[test]
fn passing_rust_tests_compact() {
    let raw = (0..10000)
        .map(|i| format!("test case_{i} ... ok\n"))
        .collect::<String>();
    let out = String::from_utf8(filtered(&["cargo-test"], raw.as_bytes())).unwrap();
    assert!(out.contains("10000 passing records"));
    assert!(out.len() < 100);
}
#[test]
fn diagnostics_survive_mixed_output() {
    let s=b"test a ... ok\nwarning: real warning\nerror[E0308]: wrong type\n  --> src/main.rs:12:3\nexpected: 401\nactual: 200\nTS2322: mismatch\npanic: unrecoverable\ntest result: FAILED. 1 passed; 1 failed\n";
    let out = String::from_utf8(filtered(&["cargo-test", "typescript"], s)).unwrap();
    for line in std::str::from_utf8(s).unwrap().lines().skip(1) {
        assert!(out.contains(line), "{line}");
    }
    assert!(!out.contains("test a ... ok"));
}
#[test]
fn unknown_output_is_byte_exact() {
    let raw = "warning: αβ\n\u{1b}[31mERROR\u{1b}[0m\nno newline".as_bytes();
    assert_eq!(filtered(&["rust"], raw), raw);
}
#[test]
fn long_lines_are_bounded_and_exact() {
    let raw = vec![b'x'; 3 * 1024 * 1024];
    assert_eq!(filtered(&["rust"], &raw), raw);
}
#[test]
fn capture_round_trip_and_cleanup() {
    let d = tempdir().unwrap();
    let s = Store::at(d.path().join("data")).unwrap();
    let cfg = Config::default();
    let mut cap = s.capture(&cfg).unwrap();
    let events = [
        OutputEvent {
            channel: Channel::Stdout,
            bytes: b"hello\0\x1b[0m".to_vec(),
        },
        OutputEvent {
            channel: Channel::Stderr,
            bytes: "αβ no newline".as_bytes().to_vec(),
        },
    ];
    for e in &events {
        cap.write(e).unwrap();
    }
    let id = cap.id.clone();
    let r = Record {
        id: id.clone(),
        created: ttc_ai::recovery::now(),
        cwd: "/tmp".into(),
        command: "cargo test".into(),
        outcome: ProcessOutcome {
            code: Some(254),
            signal: None,
        },
        raw_bytes: cap.raw_bytes,
        filtered_bytes: 0,
        duration_ms: 0,
        families: vec![],
        partial: false,
        execution_evidence: None,
    };
    s.publish(cap, &r).unwrap();
    let got = s
        .reader(&id)
        .unwrap()
        .collect::<anyhow::Result<Vec<_>>>()
        .unwrap();
    for (a, b) in events.iter().zip(got) {
        assert_eq!(a.channel, b.channel);
        assert_eq!(a.bytes, b.bytes);
    }
    assert_eq!(s.record(&id).unwrap().outcome.code, Some(254));
    assert_eq!(s.clean(&cfg, true).unwrap(), 1);
    assert!(s.reader(&id).is_err());
}
#[test]
fn legacy_capture_record_without_execution_evidence_still_deserializes() {
    let value = json!({
        "id": "0123456789abcdef0123456789abcdef",
        "created": 1,
        "cwd": "/tmp",
        "command": "cargo test",
        "outcome": {"code": 0, "signal": null},
        "raw_bytes": 0,
        "filtered_bytes": 0,
        "duration_ms": 0,
        "families": ["cargo-test"],
        "partial": false
    });
    let record: Record = serde_json::from_value(value).unwrap();
    assert!(record.execution_evidence.is_none());
}
#[test]
fn capture_limit_rejects_before_write() {
    let d = tempdir().unwrap();
    let s = Store::at(d.path().join("data")).unwrap();
    let mut cfg = Config::default();
    cfg.recovery.max_capture_mb = 0;
    let mut cap = s.capture(&cfg).unwrap();
    assert!(
        cap.write(&OutputEvent {
            channel: Channel::Stdout,
            bytes: vec![1]
        })
        .is_err()
    );
    assert_eq!(cap.raw_bytes, 0);
}
#[test]
fn invalid_ids_cannot_traverse() {
    for s in ["../secret", "/etc/passwd", "xyz", "", &"g".repeat(32)] {
        assert!(ttc_ai::recovery::validate_id(s).is_err());
    }
}
#[test]
fn hook_errors_and_unknowns_fail_open() {
    let d = tempdir().unwrap();
    for s in [
        "invalid",
        "{}",
        r#"{"tool_name":"Bash","tool_input":{"command":"rm -rf /"},"cwd":"/tmp","hook_event_name":"PreToolUse"}"#,
    ] {
        assert_eq!(integrations::hook(s, d.path()), json!({}));
    }
    for command in [
        "cargo test",
        "rm -rf ./sentinel",
        "cargo test | tee output",
        "cargo test --watch",
        "echo $(cargo test)",
        "/ttc run -- cargo test",
    ] {
        let event = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "cwd": d.path(),
            "permission_mode": "dontAsk",
            "tool_input": {"command": command}
        });
        assert_eq!(
            integrations::hook(&event.to_string(), Path::new("/ttc")),
            json!({}),
            "{command}"
        );
    }
}
#[test]
fn installer_is_idempotent_and_surgical() {
    let d = tempdir().unwrap();
    let home = d.path().join("codex");
    std::fs::create_dir(&home).unwrap();
    let conf = "# preserve comment\nmodel = 'example'\n[[hooks.PreToolUse]]\nmatcher='Bash'\n[[hooks.PreToolUse.hooks]]\ntype='command'\ncommand='other-hook'\n";
    std::fs::write(home.join("config.toml"), conf).unwrap();
    let source = d.path().join("source");
    std::fs::write(&source, b"binary").unwrap();
    integrations::install(&home, &source, true).unwrap();
    assert!(!home.join("bin/ttc").exists());
    integrations::install(&home, &source, false).unwrap();
    let first = std::fs::read(home.join("config.toml")).unwrap();
    integrations::install(&home, &source, false).unwrap();
    assert_eq!(first, std::fs::read(home.join("config.toml")).unwrap());
    integrations::uninstall(&home, false).unwrap();
    let text = std::fs::read_to_string(home.join("config.toml")).unwrap();
    assert!(text.contains("# preserve comment"));
    assert!(text.contains("other-hook"));
    assert!(!text.contains("hook codex"));
    assert!(!home.join("bin/ttc").exists());
}
#[test]
fn installer_preserves_json_hooks() {
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("hooks.json"),
        json!({"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"command":"other"}]}]}})
            .to_string(),
    )
    .unwrap();
    let source = d.path().join("source");
    std::fs::write(&source, b"binary").unwrap();
    integrations::install(d.path(), &source, false).unwrap();
    integrations::uninstall(d.path(), false).unwrap();
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("hooks.json")).unwrap()).unwrap();
    assert_eq!(v["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "other");
}
proptest::proptest! {
    #[test] fn quoting_roundtrips(s in "[^\x00]{0,1000}"){let q=ttc_ai::command::quote(&s);proptest::prop_assert_eq!(shell_words::split(&q).unwrap(),vec![s]);}
    #[test] fn malformed_shell_never_panics(s in ".{0,2000}"){let _=parse(&s,Shell::Posix);}
    #[test] fn malformed_hook_never_panics(s in ".{0,2000}"){let _=integrations::hook(&s,Path::new("/ttc"));}
    #[test] fn ids_are_strict(s in ".{0,100}"){if ttc_ai::recovery::validate_id(&s).is_ok(){proptest::prop_assert_eq!(s.len(),32);proptest::prop_assert!(s.bytes().all(|b|b.is_ascii_hexdigit()));}}
}

#[test]
fn environment_prefixes_resolve_without_execution() {
    let d = tempdir().unwrap();
    package(d.path(), json!({"test":"cargo test"}));
    for s in [
        "NODE_ENV=test pnpm test",
        "env NODE_ENV=test pnpm test",
        "cross-env NODE_ENV=test pnpm test",
        "timeout 30s cargo test",
    ] {
        let c = classify(s, d.path(), Shell::Posix);
        assert!(c.filterable, "{s}: {c:?}");
        assert_eq!(c.families, vec!["cargo-test"]);
    }
}
#[test]
fn static_tasks_resolve() {
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("Makefile"),
        "test:\n\tcargo test\n\tgo test ./...\n",
    )
    .unwrap();
    let c = classify("make test", d.path(), Shell::Posix);
    assert!(c.filterable, "{c:?}");
    assert_eq!(c.families, vec!["cargo-test", "go-test"]);
    std::fs::write(d.path().join("Makefile"), "test:\n\t$(RUN)\n").unwrap();
    assert!(!classify("make test", d.path(), Shell::Posix).filterable);
}
#[test]
fn static_taskfile_resolves() {
    let d = tempdir().unwrap();
    std::fs::write(
        d.path().join("Taskfile.yml"),
        "version: '3'\ntasks:\n  check:\n    cmds: [cargo check, go vet ./...]\n",
    )
    .unwrap();
    let c = classify("task check", d.path(), Shell::Posix);
    assert!(c.filterable, "{c:?}");
    assert_eq!(c.families, vec!["go", "rust"]);
}
#[test]
fn concurrent_reservations_enforce_total_quota() {
    let d = tempdir().unwrap();
    let store = Store::at(d.path().join("data")).unwrap();
    let mut cfg = Config::default();
    cfg.recovery.max_total_mb = 80;
    let first = store.capture(&cfg).unwrap();
    assert!(store.capture(&cfg).is_err());
    drop(first);
    assert!(store.capture(&cfg).is_ok());
}
#[cfg(unix)]
#[test]
fn private_permissions_and_symlink_rejection() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let d = tempdir().unwrap();
    let store = Store::at(d.path().join("data")).unwrap();
    assert_eq!(
        std::fs::metadata(&store.root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(store.root.join("history.sqlite3"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    symlink(&store.root, d.path().join("link")).unwrap();
    assert!(Store::at(d.path().join("link")).is_err());
}
#[test]
fn every_semantic_rule_preserves_warnings_failures_and_unknowns() {
    use ttc_ai::filtering::rules::{RULES, match_line};
    for r in RULES {
        let families = vec![r.family.to_owned()];
        assert!(
            match_line(&families, r.example).is_some(),
            "{} {}",
            r.family,
            r.example
        );
        for suffix in [
            " warning: deprecated",
            " error: fatal",
            " failure: expected 1 actual 2",
            " security vulnerability",
        ] {
            assert!(match_line(&families, &format!("{}{suffix}", r.example)).is_none());
        }
        assert!(match_line(&families, "novel diagnostic from future tool version").is_none());
    }
}
proptest::proptest! {
    #[test] fn printable_shell_never_panics(s in "[ -~]{0,3000}"){let _=parse(&s,Shell::Posix);}
}
#[test]
fn uninstall_preserves_user_replacement_binary() {
    let d = tempdir().unwrap();
    let src = d.path().join("source");
    std::fs::write(&src, "original").unwrap();
    integrations::install(d.path(), &src, false).unwrap();
    let dest = d
        .path()
        .join("bin")
        .join(if cfg!(windows) { "ttc.exe" } else { "ttc" });
    std::fs::write(&dest, "user replacement").unwrap();
    assert!(integrations::install(d.path(), &src, false).is_err());
    integrations::uninstall(d.path(), false).unwrap();
    assert_eq!(std::fs::read_to_string(dest).unwrap(), "user replacement");
}
#[test]
fn malformed_installer_config_is_nonmutating() {
    let d = tempdir().unwrap();
    std::fs::write(d.path().join("config.toml"), "hooks=5\n").unwrap();
    let src = d.path().join("source");
    std::fs::write(&src, "binary").unwrap();
    assert!(integrations::install(d.path(), &src, false).is_err());
    assert!(!d.path().join("bin").exists());
    assert_eq!(
        std::fs::read_to_string(d.path().join("config.toml")).unwrap(),
        "hooks=5\n"
    );
}
#[test]
fn persistent_graph_cache_invalidates_script_and_workspace_membership() {
    let d = tempdir().unwrap();
    let cache = d.path().join("cache");
    package(d.path(), json!({"test":"cargo test"}));
    let first = ttc_ai::resolver::classify_cached("pnpm test", d.path(), Shell::Posix, &cache);
    assert_eq!(first.families, vec!["cargo-test"]);
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
    package(d.path(), json!({"test":"pytest"}));
    assert_eq!(
        ttc_ai::resolver::classify_cached("pnpm test", d.path(), Shell::Posix, &cache).families,
        vec!["pytest"]
    );
    std::fs::write(
        d.path().join("pnpm-workspace.yaml"),
        "packages: [packages/*]",
    )
    .unwrap();
    let before = ttc_ai::resolver::classify_cached("pnpm -r test", d.path(), Shell::Posix, &cache);
    assert!(!before.families.contains(&"go-test".into()));
    package(
        &d.path().join("packages/new"),
        json!({"test":"go test ./..."}),
    );
    assert!(
        ttc_ai::resolver::classify_cached("pnpm -r test", d.path(), Shell::Posix, &cache)
            .families
            .contains(&"go-test".into())
    );
}
#[test]
fn unsafe_wrappers_are_explained_but_not_filtered() {
    for (s, f) in [
        ("sudo -u root cargo test", "cargo-test"),
        ("xargs -n 1 cargo test", "cargo-test"),
        ("docker exec app cargo test", "cargo-test"),
    ] {
        let c = classify(s, Path::new("/tmp"), Shell::Posix);
        assert!(c.families.contains(&f.to_string()), "{s}");
        assert!(!c.filterable, "{s}");
    }
}
