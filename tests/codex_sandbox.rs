mod common;

#[test]
fn real_minimum_cli_executes_hook_in_all_sandbox_modes_without_login() {
    let result = std::process::Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/scripts/m10-runtime-tests.py"
        ))
        .arg(common::ttc())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let report = String::from_utf8(result.stdout).unwrap();
    for mode in ["read-only", "workspace-write", "danger-full-access"] {
        assert!(report.contains(&format!("{mode}=real-codex-hook-runtime-pass")));
    }
    assert!(report.contains("auth=none cleanup=pass"));
}
