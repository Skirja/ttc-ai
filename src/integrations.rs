//! Thin Codex protocol adapter and ownership-aware local installer.
use crate::{
    command::{Risk, Shell, quote},
    config::{self},
    recovery::{atomic_write, now, private_dir},
    resolver::classify_cached,
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
use std::{
    fs,
    path::{Path, PathBuf},
};

const CODEX_HOOK_INPUT_LIMIT: usize = 1024 * 1024;
const CODEX_HOOK_PROTOCOL_VERSION: u32 = 2;
const CODEX_PROFILES: &str = include_str!(concat!(env!("OUT_DIR"), "/codex-profiles.json"));

#[derive(Debug, Deserialize)]
struct CodexPreToolUseEvent {
    #[serde(default)]
    hook_protocol_version: Option<u32>,
    hook_event_name: String,
    tool_name: String,
    cwd: String,
    permission_mode: String,
    tool_input: CodexToolInput,
    #[serde(default)]
    execution_context: Option<CodexExecutionContext>,
}

#[derive(Debug, Deserialize)]
struct CodexToolInput {
    command: String,
}

#[derive(Debug, Deserialize)]
struct CodexExecutionContext {
    cwd: String,
    shell: CodexShellContext,
    sandbox_permissions: String,
}

#[derive(Debug, Deserialize)]
struct CodexShellContext {
    executable: String,
    dialect: Shell,
    login: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CodexCompatibilityProfile {
    pub codex_version: String,
    pub os: String,
    pub arch: String,
    pub hook_protocol_version: u32,
    pub approval_equivalence: bool,
    pub shell_fidelity: bool,
    pub cwd_fidelity: bool,
    pub login_fidelity: bool,
    pub sandbox_fidelity: bool,
    pub competing_hook_behavior: bool,
    pub model_output_fidelity: bool,
    pub exit_status_fidelity: bool,
}

#[derive(Debug, Deserialize)]
struct CodexProfiles {
    schema_version: u32,
    profiles: Vec<CodexCompatibilityProfile>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodexCompatibilityVerdict {
    Compatible,
    Incompatible,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodexCompatibilityReport {
    pub schema_version: u32,
    pub codex_version: Option<String>,
    pub profile: Option<String>,
    pub verdict: CodexCompatibilityVerdict,
    pub hook_protocol_supported: bool,
    pub approval_equivalence: bool,
    pub shell_fidelity: bool,
    pub cwd_fidelity: bool,
    pub login_fidelity: bool,
    pub sandbox_fidelity: bool,
    pub model_output_fidelity: bool,
    pub exit_status_fidelity: bool,
    pub competing_hook_behavior: bool,
    pub automatic_rewrite_enabled: bool,
    pub blockers: Vec<String>,
}

fn valid_permission_mode(mode: &str) -> bool {
    matches!(
        mode,
        "default" | "acceptEdits" | "plan" | "dontAsk" | "bypassPermissions"
    )
}

fn parse_codex_event(input: &str) -> Option<CodexPreToolUseEvent> {
    if input.len() > CODEX_HOOK_INPUT_LIMIT {
        return None;
    }
    let event: CodexPreToolUseEvent = serde_json::from_str(input).ok()?;
    let protocol = event.hook_protocol_version.unwrap_or(1);
    if !matches!(protocol, 1 | CODEX_HOOK_PROTOCOL_VERSION)
        || event.hook_event_name != "PreToolUse"
        || event.tool_name != "Bash"
        || event.tool_input.command.is_empty()
        || !Path::new(&event.cwd).is_absolute()
        || !Path::new(&event.cwd).is_dir()
        || !valid_permission_mode(&event.permission_mode)
    {
        return None;
    }
    if let Some(context) = &event.execution_context
        && (!Path::new(&context.cwd).is_absolute()
            || !Path::new(&context.cwd).is_dir()
            || !Path::new(&context.shell.executable).is_absolute())
    {
        return None;
    }
    Some(event)
}

fn event_protocol_version(event: &CodexPreToolUseEvent) -> u32 {
    event.hook_protocol_version.unwrap_or(1)
}

fn embedded_profiles() -> Vec<CodexCompatibilityProfile> {
    serde_json::from_str::<CodexProfiles>(CODEX_PROFILES)
        .ok()
        .filter(|profiles| profiles.schema_version == 1)
        .map(|profiles| profiles.profiles)
        .unwrap_or_default()
}

fn profile_is_verified(profile: &CodexCompatibilityProfile) -> bool {
    matches!(
        profile.hook_protocol_version,
        1 | CODEX_HOOK_PROTOCOL_VERSION
    ) && profile.approval_equivalence
        && profile.shell_fidelity
        && profile.cwd_fidelity
        && profile.login_fidelity
        && profile.sandbox_fidelity
        && profile.competing_hook_behavior
        && profile.model_output_fidelity
        && profile.exit_status_fidelity
}

fn matching_profile<'a>(
    codex_version: Option<&str>,
    profiles: &'a [CodexCompatibilityProfile],
) -> Option<&'a CodexCompatibilityProfile> {
    let version = codex_version?;
    profiles.iter().find(|profile| {
        profile.codex_version == version
            && profile.os == std::env::consts::OS
            && profile.arch == std::env::consts::ARCH
            && profile_is_verified(profile)
    })
}

pub fn compatibility_report(codex_version: Option<String>) -> CodexCompatibilityReport {
    let profiles = embedded_profiles();
    let matched = matching_profile(codex_version.as_deref(), &profiles);
    let profile = matched.map(|profile| profile.codex_version.clone());
    let verified = matched.is_some();
    let blockers = if verified {
        Vec::new()
    } else {
        vec![
            "no version-specific Codex compatibility profile is verified".into(),
            "Codex hook protocol v2 with preserving rewrite semantics is not attested".into(),
            "approval, shell, cwd, login, sandbox, output, and exit fidelity are not all verified".into(),
            "no compatibility profile embeds the complete competing-hook and permission-mode matrix".into(),
        ]
    };
    CodexCompatibilityReport {
        schema_version: 1,
        codex_version,
        profile,
        verdict: if verified {
            CodexCompatibilityVerdict::Compatible
        } else {
            CodexCompatibilityVerdict::Incompatible
        },
        hook_protocol_supported: true,
        approval_equivalence: verified,
        shell_fidelity: verified,
        cwd_fidelity: verified,
        login_fidelity: verified,
        sandbox_fidelity: verified,
        model_output_fidelity: verified,
        exit_status_fidelity: verified,
        competing_hook_behavior: verified,
        automatic_rewrite_enabled: verified,
        blockers,
    }
}

fn installed_codex_version() -> Option<String> {
    std::process::Command::new("codex")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Integration boundary: the optimization engine does not depend on harness JSON.
pub trait HarnessAdapter {
    fn handle(&self, input: &str, binary: &Path) -> Value;
}
/// Codex's PreToolUse protocol adapter.
pub struct CodexAdapter;
impl HarnessAdapter for CodexAdapter {
    fn handle(&self, input: &str, binary: &Path) -> Value {
        hook(input, binary)
    }
}

/// No compatibility profile is enabled without approval-equivalence evidence.
/// Protocol compatibility alone is deliberately insufficient.
pub fn compatibility_verified() -> bool {
    compatibility_report(installed_codex_version()).automatic_rewrite_enabled
}

fn is_ttc_wrapper(command: &str, binary: &Path) -> bool {
    let binary = binary.to_string_lossy();
    command.contains(binary.as_ref()) && command.contains(" run ")
}

fn hook_with_profiles(
    input: &str,
    binary: &Path,
    codex_version: Option<&str>,
    profiles: &[CodexCompatibilityProfile],
) -> Value {
    let Some(event) = parse_codex_event(input) else {
        return json!({});
    };
    if matching_profile(codex_version, profiles)
        .is_none_or(|profile| profile.hook_protocol_version != event_protocol_version(&event))
    {
        return json!({});
    }
    let execution_cwd = event
        .execution_context
        .as_ref()
        .map_or_else(|| Path::new(&event.cwd), |context| Path::new(&context.cwd));
    let Ok(cfg) = config::load(execution_cwd) else {
        return json!({});
    };
    if !cfg.enabled || !cfg.recovery.enabled || !cfg.hooks.enabled {
        return json!({});
    }
    rewrite_event(&event, binary)
}

fn rewrite_event(event: &CodexPreToolUseEvent, binary: &Path) -> Value {
    let (execution_cwd, shell_executable, dialect, login, sandbox_permissions) = event
        .execution_context
        .as_ref()
        .map(|context| {
            (
                context.cwd.as_str(),
                context.shell.executable.as_str(),
                context.shell.dialect,
                context.shell.login,
                context.sandbox_permissions.as_str(),
            )
        })
        .unwrap_or((&event.cwd, "/bin/bash", Shell::Posix, false, "use_default"));
    if dialect != Shell::Posix
        || sandbox_permissions != "use_default"
        || is_ttc_wrapper(&event.tool_input.command, binary)
    {
        return json!({});
    }
    let execution_cwd = Path::new(execution_cwd);
    let class = classify_cached(
        &event.tool_input.command,
        execution_cwd,
        dialect,
        &config::data_dir().join("discovery"),
    );
    if !class.filterable || class.risk > Risk::Diagnostic {
        return json!({});
    }
    let mut parts = vec![
        quote(&binary.to_string_lossy()),
        "run".into(),
        "--shell".into(),
        quote(shell_executable),
        "--dialect".into(),
        "posix".into(),
        "--cwd".into(),
        quote(execution_cwd.to_string_lossy().as_ref()),
    ];
    if login {
        parts.push("--login".into());
    }
    parts.extend(["--command".into(), quote(&event.tool_input.command)]);
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": {"command": parts.join(" ")}
        }
    })
}

pub fn hook(input: &str, binary: &Path) -> Value {
    let profiles = embedded_profiles();
    let version = installed_codex_version();
    hook_with_profiles(input, binary, version.as_deref(), &profiles)
}
fn binary_name() -> &'static str {
    if cfg!(windows) { "ttc.exe" } else { "ttc" }
}
fn owned_command(home: &Path) -> String {
    format!(
        "{} hook codex",
        quote(&home.join("bin").join(binary_name()).to_string_lossy())
    )
}
pub fn install(home: &Path, source: &Path, dry: bool) -> Result<Value> {
    let dest = home.join("bin").join(binary_name());
    let config = home.join("config.toml");
    let hooks = home.join("hooks.json");
    let receipt = home.join("ttc-install.json");
    if dest.exists() && !receipt.exists() {
        bail!("{} exists without TTC ownership receipt", dest.display());
    }
    let command = owned_command(home);
    let text = if config.exists() {
        fs::read_to_string(&config)?
    } else {
        String::new()
    };
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .context("invalid Codex TOML")?;
    if doc.get("hooks").is_some_and(|h| !h.is_table()) {
        bail!("Codex hooks must be a TOML table");
    }
    let use_json = hooks.exists();
    let (target, bytes) = if use_json {
        let mut v: Value =
            serde_json::from_slice(&fs::read(&hooks)?).context("invalid hooks.json")?;
        let map = v.as_object_mut().context("hooks.json must be an object")?;
        let h = map
            .entry("hooks")
            .or_insert(json!({}))
            .as_object_mut()
            .context("hooks must be an object")?;
        let groups = h
            .entry("PreToolUse")
            .or_insert(json!([]))
            .as_array_mut()
            .context("PreToolUse must be an array")?;
        let found = groups.iter().any(|g| {
            g["hooks"]
                .as_array()
                .is_some_and(|a| a.iter().any(|h| h["command"] == command))
        });
        if !found {
            groups.push(json!({"matcher":"^Bash$","hooks":[{"type":"command","command":command,"timeout":2}]}));
        }
        (hooks, serde_json::to_vec_pretty(&v)?)
    } else {
        let found = doc
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(toml_edit::Item::as_array_of_tables)
            .is_some_and(|a| {
                a.iter().any(|g| {
                    g.get("hooks")
                        .and_then(toml_edit::Item::as_array_of_tables)
                        .is_some_and(|hs| {
                            hs.iter().any(|h| {
                                h.get("command").and_then(toml_edit::Item::as_str) == Some(&command)
                            })
                        })
                })
            });
        if !found {
            if doc.get("hooks").is_none() {
                doc["hooks"] = toml_edit::Item::Table(toml_edit::Table::new());
            }
            if doc["hooks"].get("PreToolUse").is_none() {
                doc["hooks"]["PreToolUse"] =
                    toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
            }
            let a = doc["hooks"]["PreToolUse"]
                .as_array_of_tables_mut()
                .context("unexpected PreToolUse configuration")?;
            let mut group = toml_edit::Table::new();
            group["matcher"] = toml_edit::value("^Bash$");
            let mut handler = toml_edit::Table::new();
            handler["type"] = toml_edit::value("command");
            handler["command"] = toml_edit::value(command.clone());
            handler["timeout"] = toml_edit::value(2);
            let mut handlers = toml_edit::ArrayOfTables::new();
            handlers.push(handler);
            group["hooks"] = toml_edit::Item::ArrayOfTables(handlers);
            a.push(group);
        }
        (config, doc.to_string().into_bytes())
    };
    let changed = fs::read(&target).ok().as_deref() != Some(bytes.as_slice());
    let mut result = json!({"binary":dest,"config":target,"config_changed":changed,"dry_run":dry,"hook_command":command,"automatic_rewrite_enabled":false,"trust_action":"Open /hooks in Codex and review TTC; approval-equivalence profile is not yet verified"});
    if dry {
        result["proposed_config"] = String::from_utf8_lossy(&bytes).to_string().into();
        return Ok(result);
    }
    private_dir(home)?;
    private_dir(&home.join("bin"))?;
    if changed && target.exists() {
        let backup = target.with_extension(format!("ttc-backup-{}", now()));
        if !backup.exists() {
            atomic_write(&backup, &fs::read(&target)?)?;
        }
    }
    let binbytes = fs::read(source)?;
    let old_binary = fs::read(&dest).ok();
    let old_config = fs::read(&target).ok();
    let old_receipt = fs::read(&receipt).ok();
    if let (Some(old), Some(receipt_bytes)) = (&old_binary, &old_receipt) {
        let previous: Value = serde_json::from_slice(receipt_bytes)?;
        if previous["binary_sha256"].as_str() != Some(&checksum(old)) {
            bail!("installed binary changed outside TTC; refusing overwrite");
        }
    }
    let result_write = (|| -> Result<()> {
        atomic_write(&dest, &binbytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))?;
        }
        if changed {
            atomic_write(&target, &bytes)?;
        }
        let r = json!({"schema_version":1,"binary":dest,"binary_sha256":checksum(&binbytes),"config":target,"command":command});
        atomic_write(&receipt, &serde_json::to_vec_pretty(&r)?)?;
        Ok(())
    })();
    if let Err(e) = result_write {
        for (p, previous) in [
            (&dest, old_binary),
            (&target, old_config),
            (&receipt, old_receipt),
        ] {
            if let Some(bytes) = previous {
                let _ = atomic_write(p, &bytes);
            } else {
                let _ = fs::remove_file(p);
            }
        }
        return Err(e).context("installation failed; attempted rollback");
    }
    Ok(result)
}
pub fn uninstall(home: &Path, dry: bool) -> Result<Value> {
    let receipt = home.join("ttc-install.json");
    if !receipt.exists() {
        return Ok(json!({"removed":false,"reason":"no TTC ownership receipt"}));
    }
    let r: Value = serde_json::from_slice(&fs::read(&receipt)?)?;
    let target = PathBuf::from(r["config"].as_str().context("invalid receipt")?);
    let dest = PathBuf::from(r["binary"].as_str().context("invalid receipt")?);
    let command = r["command"].as_str().context("invalid receipt")?;
    if dest != home.join("bin").join(binary_name())
        || ![home.join("config.toml"), home.join("hooks.json")].contains(&target)
        || command != owned_command(home)
    {
        bail!("receipt paths do not match TTC-owned paths");
    }
    if target.exists() {
        let bytes = if target.extension().is_some_and(|e| e == "json") {
            let mut v: Value = serde_json::from_slice(&fs::read(&target)?)?;
            if let Some(groups) = v["hooks"]["PreToolUse"].as_array_mut() {
                for g in groups.iter_mut() {
                    if let Some(h) = g["hooks"].as_array_mut() {
                        h.retain(|h| h["command"] != command);
                    }
                }
                groups.retain(|g| g["hooks"].as_array().is_none_or(|h| !h.is_empty()));
            }
            serde_json::to_vec_pretty(&v)?
        } else {
            let mut doc = fs::read_to_string(&target)?.parse::<toml_edit::DocumentMut>()?;
            if let Some(groups) = doc
                .get_mut("hooks")
                .and_then(|h| h.get_mut("PreToolUse"))
                .and_then(toml_edit::Item::as_array_of_tables_mut)
            {
                for g in groups.iter_mut() {
                    if let Some(h) = g
                        .get_mut("hooks")
                        .and_then(toml_edit::Item::as_array_of_tables_mut)
                    {
                        h.retain(|h| {
                            h.get("command").and_then(toml_edit::Item::as_str) != Some(command)
                        });
                    }
                }
                groups.retain(|g| {
                    g.get("hooks")
                        .and_then(toml_edit::Item::as_array_of_tables)
                        .is_none_or(|h| !h.is_empty())
                });
            }
            doc.to_string().into_bytes()
        };
        if !dry {
            atomic_write(&target, &bytes)?;
        }
    }
    let binary_owned =
        fs::read(&dest).is_ok_and(|b| r["binary_sha256"].as_str() == Some(&checksum(&b)));
    if !dry {
        if dest.exists() && binary_owned {
            fs::remove_file(&dest)?;
        }
        fs::remove_file(receipt)?;
    }
    Ok(
        json!({"removed":!dry,"dry_run":dry,"config":target,"modified_binary_preserved":!binary_owned && dest.exists()}),
    )
}
pub fn doctor() -> Value {
    let home = config::codex_home();
    let store = crate::recovery::Store::open();
    let writable = store.as_ref().is_ok_and(|s| s.status().is_ok());
    let config_text = fs::read_to_string(home.join("config.toml"));
    let parsed = config_text
        .as_ref()
        .ok()
        .and_then(|s| s.parse::<toml::Value>().ok());
    let config_valid = !home.join("config.toml").exists() || parsed.is_some();
    let hooks_json = fs::read(home.join("hooks.json"))
        .ok()
        .and_then(|s| serde_json::from_slice::<Value>(&s).ok());
    fn contains_command(v: &Value, command: &str) -> bool {
        match v {
            Value::Object(m) => {
                m.get("command").and_then(Value::as_str) == Some(command)
                    || m.values().any(|v| contains_command(v, command))
            }
            Value::Array(a) => a.iter().any(|v| contains_command(v, command)),
            _ => false,
        }
    }
    let command = owned_command(&home);
    let registered = hooks_json
        .as_ref()
        .is_some_and(|v| contains_command(v, &command))
        || parsed
            .as_ref()
            .and_then(|v| serde_json::to_value(v).ok())
            .is_some_and(|v| contains_command(&v, &command));
    let bin = home.join("bin").join(binary_name());
    let hook_works = std::process::Command::new(&bin)
        .args(["hook", "codex"])
        .stdin(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success() && serde_json::from_slice::<Value>(&o.stdout).is_ok());
    let hooks_enabled = parsed
        .as_ref()
        .and_then(|v| v.get("features"))
        .and_then(|v| v.get("hooks"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(true);
    let codex_version = installed_codex_version();
    let compatibility = compatibility_report(codex_version.clone());
    let ttc_config = config::load(&std::env::current_dir().unwrap_or_default()).ok();
    let ttc_hook_enabled = ttc_config
        .as_ref()
        .is_some_and(|cfg| cfg.enabled && cfg.recovery.enabled && cfg.hooks.enabled);
    let compatible = compatibility.automatic_rewrite_enabled;
    json!({"schema_version":3,"ttc_version":env!("CARGO_PKG_VERSION"),"binary":std::env::current_exe().ok(),"codex_version":codex_version,"codex_compatibility":compatibility,"installed_binary":bin.exists(),"receipt_present":home.join("ttc-install.json").exists(),"config_valid":config_valid,"hooks_json_valid":!home.join("hooks.json").exists()||hooks_json.is_some(),"hooks_feature_enabled":hooks_enabled,"ttc_hook_enabled":ttc_hook_enabled,"hook_registered":registered,"hook_executable_works":hook_works,"recovery_writable":writable,"compatible":compatible,"enabled":ttc_hook_enabled,"automatic_rewrite_active":compatible && ttc_hook_enabled && registered && hook_works,"automatic_rewrite_enabled":compatible && ttc_hook_enabled,"limitation":if compatible {"automatic rewrite requires explicit TTC hook enablement and a trusted registered hook"} else {"no exact Codex hook protocol v2 compatibility profile is verified"},"trust_action":"/hooks"})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_codex_event_requires_complete_supported_metadata() {
        let valid = r#"{"hook_protocol_version":2,"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"dontAsk","tool_input":{"command":"cargo test","future":true},"execution_context":{"cwd":"/tmp","shell":{"executable":"/bin/bash","dialect":"posix","login":false},"sandbox_permissions":"use_default"},"future":"retained"}"#;
        let event = parse_codex_event(valid).expect("valid event");
        assert_eq!(event.tool_input.command, "cargo test");
        for invalid in [
            "{}",
            r#"{"hook_protocol_version":2,"hook_event_name":"PostToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"dontAsk","tool_input":{"command":"cargo test"},"execution_context":{"cwd":"/tmp","shell":{"executable":"/bin/bash","dialect":"posix","login":false},"sandbox_permissions":"use_default"}}"#,
            r#"{"hook_protocol_version":2,"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"relative","permission_mode":"dontAsk","tool_input":{"command":"cargo test"},"execution_context":{"cwd":"/tmp","shell":{"executable":"/bin/bash","dialect":"posix","login":false},"sandbox_permissions":"use_default"}}"#,
            r#"{"hook_protocol_version":2,"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"unexpected","tool_input":{"command":"cargo test"},"execution_context":{"cwd":"/tmp","shell":{"executable":"/bin/bash","dialect":"posix","login":false},"sandbox_permissions":"use_default"}}"#,
        ] {
            assert!(parse_codex_event(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn oversized_codex_event_is_fail_open() {
        let oversized = "x".repeat(CODEX_HOOK_INPUT_LIMIT + 1);
        assert!(parse_codex_event(&oversized).is_none());
        assert_eq!(hook(&oversized, Path::new("/ttc")), json!({}));
    }

    #[test]
    fn compatibility_is_fail_closed_without_an_exact_profile() {
        let report = compatibility_report(Some("codex-cli 0.154.0-alpha.6.2".into()));
        assert_eq!(report.verdict, CodexCompatibilityVerdict::Incompatible);
        assert!(!report.automatic_rewrite_enabled);
        assert!(!report.approval_equivalence);
        assert!(!report.shell_fidelity);
        assert!(!report.cwd_fidelity);
        assert!(!report.login_fidelity);
        assert!(!report.sandbox_fidelity);
        assert!(!report.model_output_fidelity);
        assert!(!report.exit_status_fidelity);
        assert!(!report.competing_hook_behavior);
        assert!(!report.blockers.is_empty());
        assert!(!compatibility_verified());
    }

    #[test]
    fn verified_profile_requires_every_invariant_and_exact_platform() {
        let mut profile = verified_test_profile();
        assert!(matching_profile(Some("codex-cli test"), &[profile.clone()]).is_some());
        profile.login_fidelity = false;
        assert!(matching_profile(Some("codex-cli test"), &[profile]).is_none());
        assert!(matching_profile(Some("other"), &[verified_test_profile()]).is_none());
    }

    #[test]
    fn eligible_v2_event_serializes_preserving_rewrite() {
        let input = r#"{"hook_protocol_version":2,"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"dontAsk","tool_input":{"command":"cargo test"},"execution_context":{"cwd":"/tmp","shell":{"executable":"/bin/bash","dialect":"posix","login":true},"sandbox_permissions":"use_default"}}"#;
        let event = parse_codex_event(input).unwrap();
        let output = rewrite_event(&event, Path::new("/opt/ttc/bin/ttc"));
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "allow");
        let command = output["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        assert!(command.contains("'cargo test'"));
        assert!(command.contains("--shell '/bin/bash'"));
        assert!(command.contains("--cwd '/tmp'"));
        assert!(command.contains("--login"));
    }

    #[test]
    fn official_codex_event_uses_session_cwd_and_allow_rewrite() {
        let input = r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"default","tool_input":{"command":"cargo test"}}"#;
        let event = parse_codex_event(input).unwrap();
        let output = rewrite_event(&event, Path::new("/opt/ttc/bin/ttc"));
        assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "allow");
        let command = output["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .unwrap();
        assert!(command.contains("--shell '/bin/bash'"));
        assert!(command.contains("--cwd '/tmp'"));
    }

    #[test]
    fn unsafe_or_recursive_events_are_not_rewritten() {
        for command in [
            "rm -rf ./sentinel",
            "cargo test | tee output",
            "/ttc run -- cargo test",
        ] {
            let input = format!(
                r#"{{"hook_protocol_version":2,"hook_event_name":"PreToolUse","tool_name":"Bash","cwd":"/tmp","permission_mode":"dontAsk","tool_input":{{"command":{}}},"execution_context":{{"cwd":"/tmp","shell":{{"executable":"/bin/bash","dialect":"posix","login":false}},"sandbox_permissions":"use_default"}}}}"#,
                serde_json::to_string(command).unwrap()
            );
            let event = parse_codex_event(&input).unwrap();
            assert_eq!(
                rewrite_event(&event, Path::new("/ttc")),
                json!({}),
                "{command}"
            );
        }
    }

    fn verified_test_profile() -> CodexCompatibilityProfile {
        CodexCompatibilityProfile {
            codex_version: "codex-cli test".into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            hook_protocol_version: CODEX_HOOK_PROTOCOL_VERSION,
            approval_equivalence: true,
            shell_fidelity: true,
            cwd_fidelity: true,
            login_fidelity: true,
            sandbox_fidelity: true,
            competing_hook_behavior: true,
            model_output_fidelity: true,
            exit_status_fidelity: true,
        }
    }
}
