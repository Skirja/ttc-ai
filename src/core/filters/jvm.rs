use super::super::classification::Family;

pub(super) fn recognize(family: &Family, line: &str) -> bool {
    match family {
        Family::JvmTest => jvm_test_pass(line),
        Family::JvmBuild | Family::JvmProgress => maven_download(line) || gradle_task(line),
        _ => false,
    }
}

fn jvm_test_pass(line: &str) -> bool {
    let text = line.trim();
    if text
        .chars()
        .next()
        .is_some_and(|ch| matches!(ch, '│' | '├' | '└' | '─'))
    {
        return text.ends_with('✔') || text.ends_with('✓');
    }
    ["✔ ", "✓ ", "√ "]
        .iter()
        .any(|mark| text.strip_prefix(mark).is_some_and(|name| !name.is_empty()))
}

fn maven_download(line: &str) -> bool {
    let text = line.trim();
    if let Some(rest) = text.strip_prefix("[INFO] Downloading from ") {
        return rest.contains(':') && rest.len() <= 2048;
    }
    if let Some(rest) = text.strip_prefix("[INFO] Downloaded from ") {
        return rest.contains(": ")
            && rest.contains(" (")
            && rest.ends_with(')')
            && rest.len() <= 2048;
    }
    false
}

fn gradle_task(line: &str) -> bool {
    let text = line.trim();
    let Some(task) = text.strip_prefix("> Task :") else {
        return false;
    };
    if task.is_empty() || task.len() > 512 {
        return false;
    }
    let mut parts = task.split_whitespace();
    let Some(name) = parts.next() else {
        return false;
    };
    if parts.next().is_some_and(|status| {
        !matches!(
            status,
            "UP-TO-DATE" | "FROM-CACHE" | "SKIPPED" | "NO-SOURCE"
        )
    }) || parts.next().is_some()
    {
        return false;
    }
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.'))
}
