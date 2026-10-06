use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use serde::Deserialize;
use serde_json::json;

const LIMIT: u64 = 1024 * 1024;

#[derive(Deserialize)]
struct Event {
    hook_event_name: String,
    tool_name: String,
    cwd: String,
    tool_input: Input,
}

#[derive(Deserialize)]
struct Input {
    command: Option<String>,
}

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

pub(crate) fn run() -> ExitCode {
    let result = (|| -> Result<(), String> {
        let mut bytes = Vec::new();
        io::stdin()
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "stdin hook tidak dapat dibaca")?;
        if bytes.len() as u64 > LIMIT {
            return Err("Input hook melebihi 1 MiB".into());
        }
        let event: Event =
            serde_json::from_slice(&bytes).map_err(|_| "Payload hook JSON tidak valid")?;
        if event.hook_event_name != "PreToolUse"
            || !Path::new(&event.cwd).is_absolute()
            || event.cwd.contains('\0')
        {
            return Err("Event atau cwd hook tidak valid".into());
        }
        let executable = std::env::current_exe().map_err(|_| "Path binary hook tidak tersedia")?;
        let executable = executable.to_str().ok_or("Path binary hook bukan UTF-8")?;
        let response = if event.tool_name != "Bash" {
            json!({})
        } else {
            let command = event
                .tool_input
                .command
                .ok_or("command Bash wajib string")?;
            if command.contains('\0') {
                return Err("command Bash mengandung NUL".into());
            }
            if recursive(&command, executable) {
                json!({})
            } else {
                json!({"hookSpecificOutput": {
                    "hookEventName": "PreToolUse", "permissionDecision": "allow",
                    "updatedInput": {"command": format!("{} {}", quote(executable), quote(&command))}
                }})
            }
        };
        let mut bytes = serde_json::to_vec(&response).map_err(|_| "Response hook gagal")?;
        bytes.push(b'\n');
        io::stdout()
            .lock()
            .write_all(&bytes)
            .map_err(|_| "stdout hook gagal")?;
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ttc hook codex: {error}");
            ExitCode::from(2)
        }
    }
}

// This lexer proves literal command positions only. It never evaluates shell.
pub(super) fn literals(command: &str) -> Option<Vec<Option<String>>> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if q == c => quote = None,
            (Some('\''), c) => word.push(c),
            (Some('"'), '$' | '`') => return None,
            (Some('"'), '\\') => {
                let next = chars.next()?;
                if matches!(next, '$' | '`' | '"' | '\\') {
                    word.push(next);
                } else if next != '\n' {
                    word.push('\\');
                    word.push(next);
                }
            }
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(ch);
                started = true;
            }
            (None, '\\') => {
                let next = chars.next()?;
                if next != '\n' {
                    word.push(next);
                    started = true;
                }
            }
            (None, '$' | '`' | '(' | ')' | '<' | '>' | '{' | '}') => return None,
            (None, '#') if !started => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
                tokens.push(None);
            }
            (None, ' ' | '\t' | '\r') => {
                if started {
                    tokens.push(Some(std::mem::take(&mut word)));
                    started = false;
                }
            }
            (None, ';' | '&' | '|' | '\n') => {
                if started {
                    tokens.push(Some(std::mem::take(&mut word)));
                    started = false;
                }
                tokens.push(None);
            }
            (None, c) => {
                word.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return None;
    }
    if started {
        tokens.push(Some(word));
    }
    Some(tokens)
}

fn assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .bytes()
            .enumerate()
            .all(|(i, ch)| ch == b'_' || ch.is_ascii_alphabetic() || (i > 0 && ch.is_ascii_digit()))
}

pub(super) fn recursive(command: &str, executable: &str) -> bool {
    let Some(tokens) = literals(command) else {
        return false;
    };
    let mut head = true;
    let mut prefix = false;
    for token in tokens {
        let Some(word) = token else {
            head = true;
            prefix = false;
            continue;
        };
        if !head {
            continue;
        }
        if assignment(&word) || matches!(word.as_str(), "command" | "exec") {
            prefix = true;
            continue;
        }
        if prefix && matches!(word.as_str(), "--" | "-p") {
            continue;
        }
        if word == "ttc" || word == executable {
            return true;
        }
        head = false;
    }
    false
}
