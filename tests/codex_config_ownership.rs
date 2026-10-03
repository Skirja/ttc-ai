mod common;
use common::codex::Fixture;
use std::os::unix::fs::{PermissionsExt, symlink};

#[test]
fn toml_comments_inline_and_array_tables_round_trip_exactly() {
    let configs = [
        "# user\nmodel = 'chosen'\n",
        "hooks = { PreToolUse = [] } # user\n",
        "[hooks]\nPreToolUse = [{ matcher='other', hooks=[{type='command',command='echo other'}] }]\n",
        "[[hooks.PreToolUse]]\nmatcher = 'other' # keep\n[[hooks.PreToolUse.hooks]]\ntype='command'\ncommand='echo other'\n",
    ];
    for original in configs {
        let fixture = Fixture::new();
        fixture.write(&fixture.config(), original);
        fixture.install();
        fixture.uninstall();
        assert_eq!(std::fs::read_to_string(fixture.config()).unwrap(), original);
    }
}

#[test]
fn codex_trust_state_survives_install_and_uninstall() {
    let fixture = Fixture::new();
    let state = "[hooks.state.user_hook]\ntrusted_hash = 'user-hash'\n";
    fixture.write(&fixture.config(), state);
    fixture.install();
    let mut text = std::fs::read_to_string(fixture.config()).unwrap();
    text.push_str("\n[hooks.state.ttc_test_hook]\ntrusted_hash = 'codex-generated-hash'\n");
    std::fs::write(fixture.config(), text).unwrap();
    fixture.install();
    fixture.uninstall();
    let text = std::fs::read_to_string(fixture.config()).unwrap();
    assert!(text.contains("'user-hash'") && text.contains("'codex-generated-hash'"));
    assert!(!text.contains("hook codex"));
}

#[test]
fn json_whitespace_and_existing_groups_round_trip_exactly() {
    for original in [
        " { } \n",
        "{ \"hooks\": { \"PreToolUse\": [] }, \"extra\":123 }\n",
        "{\"hooks\":{\"PreToolUse\":[{\"matcher\":\"other\",\"hooks\":[]}]}}",
    ] {
        let fixture = Fixture::new();
        fixture.write(&fixture.json(), original);
        fixture.install();
        fixture.uninstall();
        assert_eq!(std::fs::read_to_string(fixture.json()).unwrap(), original);
    }
}

#[test]
fn user_edits_after_install_and_new_hooks_survive_uninstall() {
    let fixture = Fixture::new();
    fixture.install();
    let mut config = std::fs::read_to_string(fixture.config()).unwrap();
    config.push_str("\n[[hooks.PreToolUse]]\nmatcher='other'\n[[hooks.PreToolUse.hooks]]\ntype='command'\ncommand='echo preserved'\n\n[custom]\nvalue='new user setting' # keep\n");
    std::fs::write(fixture.config(), config).unwrap();
    fixture.uninstall();
    let after = std::fs::read_to_string(fixture.config()).unwrap();
    assert!(after.contains("echo preserved") && after.contains("new user setting"));
    assert!(!after.contains("hook codex"));
    let fixture = Fixture::new();
    fixture.write(&fixture.json(), "{}");
    fixture.install();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.json()).unwrap()).unwrap();
    value["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"matcher":"other","hooks":[]}));
    value["new"] = serde_json::json!(123);
    std::fs::write(fixture.json(), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    fixture.uninstall();
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture.json()).unwrap()).unwrap();
    assert_eq!(value["new"], 123);
    assert_eq!(value["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
}

#[test]
fn changed_or_unowned_ttc_hook_is_preserved_and_refused() {
    let fixture = Fixture::new();
    fixture.install();
    let config = std::fs::read_to_string(fixture.config())
        .unwrap()
        .replace("timeout = 10", "timeout = 11");
    std::fs::write(fixture.config(), &config).unwrap();
    assert!(!fixture.run(&["install", "codex"]).status.success());
    assert!(!fixture.run(&["uninstall", "codex"]).status.success());
    assert_eq!(std::fs::read_to_string(fixture.config()).unwrap(), config);
    assert!(fixture.data().join("codex.toml").exists());
    let fixture = Fixture::new();
    fixture.install();
    std::fs::remove_file(fixture.data().join("codex.toml")).unwrap();
    assert!(!fixture.run(&["install", "codex"]).status.success());
}

#[test]
fn unsafe_or_invalid_config_is_never_replaced() {
    for text in ["broken = [", "hooks='invalid'", "hooks = {PreToolUse=[{}]}"] {
        let fixture = Fixture::new();
        fixture.write(&fixture.config(), text);
        assert!(!fixture.run(&["install", "codex"]).status.success());
        assert_eq!(std::fs::read_to_string(fixture.config()).unwrap(), text);
    }
    for text in ["{bad}", "{\"hooks\":{},\"hooks\":{}}", "[]"] {
        let fixture = Fixture::new();
        fixture.write(&fixture.json(), text);
        assert!(!fixture.run(&["install", "codex"]).status.success());
        assert_eq!(std::fs::read_to_string(fixture.json()).unwrap(), text);
    }
    let fixture = Fixture::new();
    fixture.write(&fixture.config(), "model='preserved'");
    std::fs::set_permissions(fixture.config(), std::fs::Permissions::from_mode(0o400)).unwrap();
    assert!(!fixture.run(&["install", "codex"]).status.success());
    let fixture = Fixture::new();
    let target = fixture.root.path().join("user-file");
    fixture.write(&target, "preserved");
    std::fs::create_dir_all(fixture.config().parent().unwrap()).unwrap();
    symlink(&target, fixture.config()).unwrap();
    assert!(!fixture.run(&["install", "codex"]).status.success());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "preserved");
}

#[test]
fn receipt_corruption_and_pending_transaction_block_mutation() {
    let fixture = Fixture::new();
    fixture.install();
    let config = std::fs::read(fixture.config()).unwrap();
    std::fs::write(fixture.data().join("codex.toml"), "owner='foreign'").unwrap();
    assert!(!fixture.run(&["uninstall", "codex"]).status.success());
    assert_eq!(std::fs::read(fixture.config()).unwrap(), config);
    let fixture = Fixture::new();
    std::fs::write(fixture.data().join("install.pending"), "interrupted").unwrap();
    assert!(!fixture.run(&["install", "codex"]).status.success());
    assert!(!fixture.config().exists());
}
