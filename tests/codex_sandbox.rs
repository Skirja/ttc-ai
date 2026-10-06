mod common;

#[test]
fn real_minimum_cli_executes_hook_in_all_sandbox_modes_without_login() {
    let binary = std::env::var_os("TTC_M10_TEST_BINARY")
        .expect("Codex runtime test must use the production release-mode TTC binary");
    let result = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/m10-runtime-tests.py"
        ))
        .arg(binary)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let report = String::from_utf8(result.stdout).unwrap();
    let modes: &[&str] = if std::env::var("TTC_M10_CI_RUNTIME").as_deref() == Ok("1") {
        &["read-only"]
    } else {
        &["read-only", "workspace-write", "danger-full-access"]
    };
    for mode in modes {
        assert!(report.contains(&format!("{mode}=real-codex-hook-runtime-pass")));
    }
    assert!(report.contains("auth=none cleanup=pass"));
}
