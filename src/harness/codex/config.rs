//! Surgical JSON edits and formatting-preserving TOML edits.

use serde_json::{Value, json};
use toml_edit::{DocumentMut, Item};

type Result<T> = std::result::Result<T, String>;

pub(super) fn group(binary: &str) -> Value {
    json!({"matcher":"^Bash$","hooks":[{"type":"command", "command":format!("{} hook codex",super::hook::quote(binary)), "timeout":10,"statusMessage":"TTC Bash output"}]})
}

pub(super) fn parse(text: &str, format: &str) -> Result<Value> {
    let value = match format {
        "json" => {
            Node::parse(text)?;
            serde_json::from_str(text).map_err(|_| "Config JSON tidak valid")?
        }
        "toml" => {
            text.parse::<DocumentMut>()
                .map_err(|_| "Config TOML tidak valid")?;
            let table: toml::Table = toml::from_str(text).map_err(|_| "Config TOML tidak valid")?;
            serde_json::to_value(table).map_err(|_| "Config TOML tidak dapat dibaca")?
        }
        _ => return Err("Format receipt tidak dikenal".into()),
    };
    if !value.is_object() {
        return Err("Config harus object/table".into());
    }
    if let Some(hooks) = value.get("hooks") {
        let hooks = hooks.as_object().ok_or("hooks harus object/table")?;
        for (event, groups) in hooks {
            if !matches!(
                event.as_str(),
                "PreToolUse"
                    | "PermissionRequest"
                    | "PostToolUse"
                    | "PreCompact"
                    | "PostCompact"
                    | "SessionStart"
                    | "SessionEnd"
                    | "UserPromptSubmit"
                    | "SubagentStart"
                    | "SubagentStop"
                    | "Stop"
                    | "Interrupt"
            ) {
                continue;
            }
            let groups = groups.as_array().ok_or("Event hook harus array")?;
            for group in groups {
                if !group.is_object() || !group.get("hooks").is_some_and(Value::is_array) {
                    return Err("Matcher group hook tidak valid".into());
                }
            }
        }
    }
    Ok(value)
}

fn groups(value: &Value) -> &[Value] {
    value
        .get("hooks")
        .and_then(|h| h.get("PreToolUse"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub(super) fn verify(value: &Value, owned: &Value, installed: bool) -> Result<Option<usize>> {
    let mut matched = None;
    let command = &owned["hooks"][0]["command"];
    let executable = command
        .as_str()
        .and_then(super::hook::literals)
        .and_then(|words| words.into_iter().next().flatten())
        .ok_or("Definisi executable hook invalid")?;
    for (index, group) in groups(value).iter().enumerate() {
        let related = group
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| super::hook::recursive(command, &executable))
                        || h.get("statusMessage") == Some(&json!("TTC Bash output"))
                })
            });
        if related {
            if !installed || group != owned || matched.is_some() {
                return Err(
                    "Hook TTC existing tidak dimiliki, diubah, atau ambigu; config dipertahankan"
                        .into(),
                );
            }
            matched = Some(index);
        }
    }
    if installed && matched.is_none() {
        return Err("Hook tidak cocok dengan receipt ownership".into());
    }
    Ok(matched)
}

fn one_insertion(before: &str, after: &str) -> Result<String> {
    let mut prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !before.is_char_boundary(prefix) || !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let suffix = before.as_bytes()[prefix..]
        .iter()
        .rev()
        .zip(after.as_bytes()[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    if prefix + suffix != before.len() {
        return Err("Edit memerlukan perubahan byte unrelated; config dipertahankan".into());
    }
    Ok(after[prefix..after.len() - suffix].to_owned())
}

pub(super) fn install(text: &str, format: &str, owned: &Value) -> Result<(String, String)> {
    let before = parse(text, format)?;
    verify(&before, owned, false)?;
    let after = if format == "json" {
        let root = Node::parse(text)?;
        let group = serde_json::to_string(owned).map_err(|_| "Serialisasi hook gagal")?;
        if let Some(hooks) = root.get("hooks") {
            if let Some(array) = hooks.get("PreToolUse") {
                array.append(text, &group)?
            } else {
                hooks.append(text, &format!("\"PreToolUse\":[{group}]"))?
            }
        } else {
            root.append(text, &format!("\"hooks\":{{\"PreToolUse\":[{group}]}}"))?
        }
    } else {
        let block = format!(
            "\n[[hooks.PreToolUse]]\nmatcher = \"^Bash$\"\n\n[[hooks.PreToolUse.hooks]]\ntype = \"command\"\ncommand = {}\ntimeout = 10\nstatusMessage = \"TTC Bash output\"\n",
            toml_edit::Value::from(
                owned["hooks"][0]["command"]
                    .as_str()
                    .ok_or("Command hook invalid")?
            )
        );
        let appended = format!("{text}{block}");
        if parse(&appended, format).is_ok() {
            appended
        } else {
            let mut doc = text
                .parse::<DocumentMut>()
                .map_err(|_| "Config TOML invalid")?;
            let group = inline(owned)?;
            if doc.get("hooks").is_none() {
                doc["hooks"] = Item::Table(toml_edit::Table::new());
            }
            let hooks = doc["hooks"]
                .as_table_like_mut()
                .ok_or("hooks TOML tidak dapat diedit")?;
            if !hooks.contains_key("PreToolUse") {
                hooks.insert(
                    "PreToolUse",
                    Item::Value(toml_edit::Value::Array(toml_edit::Array::new())),
                );
            }
            hooks
                .get_mut("PreToolUse")
                .and_then(Item::as_array_mut)
                .ok_or("Event TOML tidak dapat diedit")?
                .push(group);
            doc.to_string()
        }
    };
    let insertion = one_insertion(text, &after)?;
    verify(&parse(&after, format)?, owned, true)?;
    Ok((after, insertion))
}

fn inline(value: &Value) -> Result<toml_edit::Value> {
    Ok(match value {
        Value::String(value) => toml_edit::Value::from(value.as_str()),
        Value::Number(value) => {
            toml_edit::Value::from(value.as_i64().ok_or("Hook number invalid")?)
        }
        Value::Array(values) => {
            let mut array = toml_edit::Array::new();
            for value in values {
                array.push(inline(value)?);
            }
            toml_edit::Value::Array(array)
        }
        Value::Object(values) => {
            let mut table = toml_edit::InlineTable::new();
            for (key, value) in values {
                table.insert(key, inline(value)?);
            }
            toml_edit::Value::InlineTable(table)
        }
        _ => return Err("Hook type invalid".into()),
    })
}

pub(super) fn uninstall(
    text: &str,
    format: &str,
    owned: &Value,
    insertion: &str,
) -> Result<String> {
    let before = parse(text, format)?;
    let index = verify(&before, owned, true)?.ok_or("Hook tidak ada")?;
    let mut expected = before.clone();
    expected["hooks"]["PreToolUse"]
        .as_array_mut()
        .ok_or("Event hook invalid")?
        .remove(index);
    normalize(&mut expected);
    if !insertion.is_empty() && text.matches(insertion).count() == 1 {
        let candidate = text.replacen(insertion, "", 1);
        if parse(&candidate, format).is_ok_and(|mut value| {
            normalize(&mut value);
            value == expected && verify(&value, owned, false).is_ok()
        }) {
            return Ok(candidate);
        }
    }
    let after = if format == "json" {
        Node::parse(text)?
            .get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .ok_or("Event hook tidak ada")?
            .remove(text, index)?
    } else {
        let mut doc = text
            .parse::<DocumentMut>()
            .map_err(|_| "Config TOML invalid")?;
        let item = doc["hooks"]
            .as_table_like_mut()
            .and_then(|h| h.get_mut("PreToolUse"))
            .ok_or("Event hook tidak ada")?;
        if let Some(groups) = item.as_array_of_tables_mut() {
            groups.remove(index);
        } else if let Some(groups) = item.as_array_mut() {
            groups.remove(index);
        } else {
            return Err("Event hook tidak dapat diedit".into());
        }
        doc.to_string()
    };
    let mut actual = parse(&after, format)?;
    verify(&actual, owned, false)?;
    normalize(&mut actual);
    if actual != expected {
        return Err("Edit akan mengubah setting unrelated; config dipertahankan".into());
    }
    Ok(after)
}

fn normalize(value: &mut Value) {
    if let Some(hooks) = value.get_mut("hooks").and_then(Value::as_object_mut) {
        if hooks
            .get("PreToolUse")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        {
            hooks.remove("PreToolUse");
        }
        if hooks.is_empty() {
            value
                .as_object_mut()
                .expect("validated object")
                .remove("hooks");
        }
    }
}

struct Node {
    start: usize,
    end: usize,
    children: Vec<(Option<String>, Node)>,
    container: Option<u8>,
}
impl Node {
    fn get(&self, key: &str) -> Option<&Node> {
        self.children
            .iter()
            .find(|(name, _)| name.as_deref() == Some(key))
            .map(|(_, node)| node)
    }
    fn parse(text: &str) -> Result<Self> {
        let mut at = 0;
        let node = Self::read(text, &mut at, 0)?;
        Self::space(text, &mut at);
        if at != text.len() {
            return Err("Trailing JSON data".into());
        }
        Ok(node)
    }
    fn space(text: &str, at: &mut usize) {
        while text
            .as_bytes()
            .get(*at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            *at += 1;
        }
    }
    fn string(text: &str, at: &mut usize) -> Result<String> {
        let mut parser = serde_json::Deserializer::from_str(&text[*at..]).into_iter::<String>();
        let value = parser
            .next()
            .ok_or("JSON string tidak ada")?
            .map_err(|_| "JSON string invalid")?;
        *at += parser.byte_offset();
        Ok(value)
    }
    fn read(text: &str, at: &mut usize, depth: usize) -> Result<Self> {
        if depth > 128 {
            return Err("JSON nesting terlalu dalam".into());
        }
        Self::space(text, at);
        let start = *at;
        let ch = *text.as_bytes().get(*at).ok_or("JSON tidak lengkap")?;
        let mut children = Vec::new();
        let container = if matches!(ch, b'{' | b'[') {
            *at += 1;
            let close = if ch == b'{' { b'}' } else { b']' };
            let mut keys = std::collections::HashSet::new();
            Self::space(text, at);
            if text.as_bytes().get(*at) != Some(&close) {
                loop {
                    let key = if ch == b'{' {
                        let key = Self::string(text, at)?;
                        if !keys.insert(key.clone()) {
                            return Err("Duplicate JSON key".into());
                        }
                        Self::space(text, at);
                        if text.as_bytes().get(*at) != Some(&b':') {
                            return Err("JSON colon invalid".into());
                        }
                        *at += 1;
                        Some(key)
                    } else {
                        None
                    };
                    children.push((key, Self::read(text, at, depth + 1)?));
                    Self::space(text, at);
                    if text.as_bytes().get(*at) == Some(&close) {
                        break;
                    }
                    if text.as_bytes().get(*at) != Some(&b',') {
                        return Err("JSON comma invalid".into());
                    }
                    *at += 1;
                    Self::space(text, at);
                }
            }
            *at += 1;
            Some(ch)
        } else {
            let mut parser = serde_json::Deserializer::from_str(&text[*at..]).into_iter::<Value>();
            parser
                .next()
                .ok_or("JSON value tidak ada")?
                .map_err(|_| "JSON value invalid")?;
            *at += parser.byte_offset();
            None
        };
        Ok(Self {
            start,
            end: *at,
            children,
            container,
        })
    }
    fn append(&self, text: &str, added: &str) -> Result<String> {
        if self.container.is_none() {
            return Err("JSON container wajib".into());
        }
        let at = self
            .children
            .last()
            .map(|(_, n)| n.end)
            .unwrap_or(self.start + 1);
        Ok(format!(
            "{}{}{added}{}",
            &text[..at],
            if self.children.is_empty() { "" } else { "," },
            &text[at..]
        ))
    }
    fn remove(&self, text: &str, index: usize) -> Result<String> {
        let node = &self.children.get(index).ok_or("JSON group tidak ada")?.1;
        let (start, end) = if self.children.len() == 1 {
            (node.start, node.end)
        } else if index > 0 {
            (self.children[index - 1].1.end, node.end)
        } else {
            (node.start, self.children[index + 1].1.start)
        };
        Ok(format!("{}{}", &text[..start], &text[end..]))
    }
}
