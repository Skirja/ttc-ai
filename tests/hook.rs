mod common;

use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn invoke(bytes: &[u8]) -> Output {
    let mut child = common::ttc_command()
        .args(["hook", "codex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

fn event(command: &str) -> Value {
    json!({"hook_event_name":"PreToolUse", "tool_name":"Bash", "cwd":"/tmp", "tool_input":{"command":command}})
}

fn response(command: &str) -> Value {
    let result = invoke(&serde_json::to_vec(&event(command)).unwrap());
    assert!(result.status.success(), "{:?}", result);
    assert!(result.stderr.is_empty());
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn official_response_wraps_every_command_without_execution() {
    let dir = common::TestDir::new();
    let file = dir.path().join("never-created");
    let command = format!("touch '{}'", file.display());
    let result = response(&command);
    assert_eq!(result["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert_eq!(result["hookSpecificOutput"]["permissionDecision"], "allow");
    assert!(!file.exists());
    for command in [
        "cat README.md",
        "npm run test",
        "npm run dev",
        "",
        "custom-cli --json",
    ] {
        assert!(response(command)["hookSpecificOutput"]["updatedInput"]["command"].is_string());
    }
}

#[test]
fn quoting_round_trip_preserves_shell_semantics_and_runs_once() {
    let dir = common::TestDir::new();
    let commands = [
        "printf '%s\\n' 'space value' 'single'\"'\"'quote' 'double\"quote' '$literal' '日本'",
        "FOO='a b' sh -c 'printf \"%s\\n\" \"$FOO\"'; printf operator && printf 'done\\n'",
        "printf '%s\\n' \"$(printf substitution)\"\nprintf 'newline\\n' >&2; exit 7",
        "printf '\\377\\000binary\\n'; printf 'warning: exact\\n' >&2",
    ];
    for original in commands {
        let command = format!("printf x >> count; {original}");
        let wrapper = response(&command)["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap()
            .to_owned();
        let run = |text: &str| {
            let mut shell = Command::new("/bin/sh");
            shell.args(["-c", text]).current_dir(dir.path());
            for (key, value) in common::ttc_command().get_envs() {
                if let Some(value) = value {
                    shell.env(key, value);
                } else {
                    shell.env_remove(key);
                }
            }
            shell.output().unwrap()
        };
        let direct = run(&command);
        let wrapped = run(&wrapper);
        assert_eq!(direct.status.code(), wrapped.status.code());
        assert_eq!(direct.stdout, wrapped.stdout);
        assert_eq!(direct.stderr, wrapped.stderr);
        assert_eq!(std::fs::read(dir.path().join("count")).unwrap(), b"xx");
        std::fs::remove_file(dir.path().join("count")).unwrap();
    }
}

#[test]
fn literal_recursive_invocations_are_not_wrapped() {
    let quoted = format!("'{}' 'cargo test'", common::ttc().replace('\'', "'\"'\"'"));
    for command in [
        "ttc npm test",
        " FOO='a b' command -- ttc 'cargo test'",
        "exec ttc npm test",
        &quoted,
    ] {
        assert_eq!(response(command), json!({}), "{command}");
    }
    let wrapped = response("cargo test")["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(response(&wrapped), json!({}));
    for command in [
        "echo ttc",
        "printf '%s' 'ttc cargo test'",
        "echo /tmp/ttc",
        "\"$TTC\" test",
        "ttc 'unterminated",
    ] {
        assert!(
            response(command).get("hookSpecificOutput").is_some(),
            "{command}"
        );
    }
}

#[test]
fn extra_fields_and_other_tools_are_ignored() {
    let mut payload = event("cat README.md");
    payload["extra"] = json!({"unrelated":123});
    payload["tool_input"]["extra"] = json!(false);
    assert!(
        invoke(&serde_json::to_vec(&payload).unwrap())
            .status
            .success()
    );
    payload["tool_name"] = json!("apply_patch");
    payload["tool_input"] = json!({"patch":"ignored"});
    let result = invoke(&serde_json::to_vec(&payload).unwrap());
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap(),
        json!({})
    );
}

#[test]
fn invalid_payload_fails_without_partial_wrapper() {
    let mut payloads = vec![
        b"[]".to_vec(),
        b"{}".to_vec(),
        b"{bad".to_vec(),
        b"{} {}".to_vec(),
    ];
    for (key, value) in [
        ("hook_event_name", json!("PostToolUse")),
        ("cwd", json!("relative")),
        ("tool_input", json!({"command":5})),
        ("tool_input", json!({"command":"x\0y"})),
    ] {
        let mut payload = event("cat README.md");
        payload[key] = value;
        payloads.push(serde_json::to_vec(&payload).unwrap());
    }
    payloads.push(br#"{"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","tool_input":{"command":"a","command":"b"}}"#.to_vec());
    for bytes in payloads {
        let result = invoke(&bytes);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!result.stderr.is_empty());
    }
}

#[test]
fn large_event_is_bounded_and_rejected() {
    let bytes = serde_json::to_vec(&event(&"x".repeat(1024 * 1024))).unwrap();
    let result = invoke(&bytes);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
}
