mod common;
use common::codex::Fixture;

#[test]
fn installs_one_owned_hook_and_preserves_binary() {
    let fixture = Fixture::new();
    let binary = std::fs::read(fixture.home().join(".local/bin/ttc")).unwrap();
    fixture.install();
    fixture.install();
    let config = std::fs::read_to_string(fixture.config()).unwrap();
    assert_eq!(config.matches("[[hooks.PreToolUse]]").count(), 1);
    assert!(config.contains("^Bash$") && config.contains("hook codex"));
    assert!(
        std::fs::read_to_string(fixture.data().join("install.toml"))
            .unwrap()
            .contains("\"codex\"")
    );
    assert!(!fixture.run(&["uninstall"]).status.success());
    fixture.uninstall();
    fixture.uninstall();
    assert!(!fixture.config().exists());
    assert!(!fixture.data().join("codex.toml").exists());
    assert_eq!(
        std::fs::read(fixture.home().join(".local/bin/ttc")).unwrap(),
        binary
    );
    assert!(!fixture.data().join("install.pending").exists());
}

#[test]
fn unsupported_cli_or_disabled_hooks_do_not_edit_config() {
    for (version, hooks) in [
        ("0.153.9", true),
        ("0.154.0", false),
        ("bad", true),
        ("0.160.0-beta", true),
    ] {
        let fixture = Fixture::new();
        fixture.codex(version, hooks);
        assert!(!fixture.run(&["install", "codex"]).status.success());
        assert!(!fixture.config().exists());
        assert!(!fixture.data().join("codex.toml").exists());
    }
    let fixture = Fixture::new();
    fixture.codex("0.154.0", true);
    fixture.install();
}

#[test]
fn unowned_binary_changed_checksum_or_unregistered_install_is_refused() {
    let fixture = Fixture::new();
    std::fs::write(fixture.home().join(".local/bin/ttc"), b"foreign").unwrap();
    let result = fixture
        .source()
        .args(["install", "codex"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!fixture.config().exists());
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.data().join("install.toml")).unwrap();
    assert!(!fixture.run(&["install", "codex"]).status.success());
}

#[test]
fn invalid_administrative_commands_are_not_executed() {
    let fixture = Fixture::new();
    for args in [
        &["install"][..],
        &["install", "claude"],
        &["install", "codex", "extra"],
        &["hook"],
        &["hook", "claude"],
        &["uninstall", "codex", "extra"],
    ] {
        assert_eq!(fixture.run(args).status.code(), Some(2));
    }
}

#[test]
fn hooks_json_is_selected_when_existing() {
    let fixture = Fixture::new();
    fixture.write(&fixture.json(), "{\"description\":\"user hooks\"}\n");
    fixture.write(&fixture.config(), "model = 'user-choice' # keep\n");
    fixture.install();
    assert!(
        std::fs::read_to_string(fixture.json())
            .unwrap()
            .contains("PreToolUse")
    );
    assert_eq!(
        std::fs::read_to_string(fixture.config()).unwrap(),
        "model = 'user-choice' # keep\n"
    );
    fixture.uninstall();
    assert_eq!(
        std::fs::read_to_string(fixture.json()).unwrap(),
        "{\"description\":\"user hooks\"}\n"
    );
}
