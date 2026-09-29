use super::super::classification::Family;

pub(super) fn recognize(family: &Family, line: &str) -> bool {
    match family {
        Family::DotnetTest => vstest_pass(line),
        Family::DotnetRestore => restore_progress(line),
        Family::DotnetBuild => build_progress(line),
        _ => false,
    }
}

fn vstest_pass(line: &str) -> bool {
    let text = line.trim();
    let Some(rest) = text.strip_prefix("Passed ") else {
        return false;
    };
    let Some((name, duration)) = rest.rsplit_once(" [") else {
        return false;
    };
    !name.is_empty() && duration_ms(duration)
}

fn duration_ms(duration: &str) -> bool {
    let Some(value) = duration.strip_suffix(" ms]") else {
        return false;
    };
    let trimmed = value.trim();
    let value = trimmed.strip_prefix("< ").unwrap_or(trimmed);
    value.parse::<f64>().is_ok()
}

fn restore_progress(line: &str) -> bool {
    let text = line.trim();
    text == "Determining projects to restore..."
        || (text.starts_with("Restored ")
            && text.contains(" (in ")
            && (text.ends_with(" sec).") || text.ends_with(" ms).")))
}

fn build_progress(line: &str) -> bool {
    let text = line.trim();
    let Some((project, output)) = text.split_once(" -> ") else {
        return false;
    };
    !project.is_empty()
        && !output.is_empty()
        && (output.ends_with(".dll") || output.ends_with(".exe"))
        && !text.contains(['\t', ':'])
}
