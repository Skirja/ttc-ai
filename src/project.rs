//! Read-only static task discovery. Never evaluate task configuration.
use std::path::Path;
/// Return static command bodies, or None when execution-time evaluation is needed.
pub fn task_commands(tool: &str, target: &str, cwd: &Path) -> Option<Vec<String>> {
    if target.is_empty() || target.starts_with('-') {
        return None;
    }
    if tool == "task" {
        let path = ["Taskfile.yml", "Taskfile.yaml"]
            .iter()
            .map(|n| cwd.join(n))
            .find(|p| p.exists())?;
        if std::fs::metadata(&path).ok()?.len() > 1024 * 1024 {
            return None;
        }
        let v: serde_json::Value =
            serde_yaml::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        let t = &v["tasks"][target];
        if t.get("deps").is_some() || t.get("sources").is_some() || t.get("preconditions").is_some()
        {
            return None;
        }
        let a = t["cmds"].as_array()?;
        let mut out = vec![];
        for cmd in a {
            let text = cmd.as_str().or_else(|| cmd["cmd"].as_str())?;
            if text.contains("{{") {
                return None;
            }
            out.push(text.to_owned());
        }
        return Some(out);
    }
    let names = if tool == "make" {
        vec!["GNUmakefile", "makefile", "Makefile"]
    } else {
        vec!["justfile", "Justfile", ".justfile"]
    };
    let path = names.iter().map(|n| cwd.join(n)).find(|p| p.exists())?;
    if std::fs::metadata(&path).ok()?.len() > 1024 * 1024 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    if text.contains("include ")
        || text.contains("$(")
        || text.contains("${")
        || text.contains("{{")
        || text.contains("`")
    {
        return None;
    }
    let mut in_target = false;
    let mut found = false;
    let mut out = vec![];
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            in_target = false;
            if let Some((name, deps)) = line.split_once(':')
                && name.trim() == target
            {
                if !deps.trim().is_empty() {
                    return None;
                }
                in_target = true;
                found = true;
            }
        } else if in_target && !line.trim().is_empty() {
            let cmd = line.trim().strip_prefix('@').unwrap_or(line.trim());
            if cmd.starts_with('-') || cmd.starts_with('+') || cmd.ends_with('\\') {
                return None;
            }
            out.push(cmd.to_owned());
        }
    }
    if found && !out.is_empty() {
        Some(out)
    } else {
        None
    }
}
