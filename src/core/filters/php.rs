use super::super::classification::Family;

pub(super) fn recognize(family: &Family, line: &str) -> bool {
    match family {
        Family::PhpTest => {
            phpunit_progress(line)
                || line.strip_prefix("✔ ").is_some_and(|name| !name.is_empty())
                || line.strip_prefix("✓ ").is_some_and(|name| !name.is_empty())
                || line
                    .strip_prefix("[PASS] ")
                    .is_some_and(|name| !name.is_empty())
        }
        Family::PhpLint => phpcs_progress(line) || phpstan_progress(line),
        Family::PhpInstall => composer_progress(line),
        _ => false,
    }
}

fn phpunit_progress(line: &str) -> bool {
    let text = line.trim();
    let Some((dots, progress)) = text.split_once(' ') else {
        return text.len() >= 4 && text.bytes().all(|byte| byte == b'.');
    };
    if dots.len() < 4 || !dots.bytes().all(|byte| byte == b'.') {
        return false;
    }
    let Some((counts, percentage)) = progress.rsplit_once('(') else {
        return false;
    };
    let Some((completed, total)) = counts.trim().split_once('/') else {
        return false;
    };
    let (Ok(completed), Ok(total)) = (completed.trim().parse::<u32>(), total.trim().parse::<u32>())
    else {
        return false;
    };
    let Some(percentage) = percentage
        .trim()
        .strip_suffix("%)")
        .and_then(|value| value.trim().parse::<u8>().ok())
    else {
        return false;
    };
    total > 0 && completed <= total && percentage <= 100
}

fn phpcs_progress(line: &str) -> bool {
    let Some((dots, count)) = line.trim().split_once(' ') else {
        return false;
    };
    dots.len() >= 4
        && dots.bytes().all(|byte| byte == b'.')
        && count.split_once('/').is_some_and(|(done, total)| {
            done.trim().parse::<u32>().is_ok() && total.trim().parse::<u32>().is_ok()
        })
}

fn phpstan_progress(line: &str) -> bool {
    let text = line.trim();
    let Some(progress) = text
        .strip_prefix('[')
        .and_then(|text| text.strip_suffix(']'))
    else {
        return false;
    };
    let Some((done, total)) = progress.split_once('/') else {
        return false;
    };
    done.trim().parse::<u32>().is_ok() && total.trim().parse::<u32>().is_ok()
}

fn composer_progress(line: &str) -> bool {
    let text = line.trim();
    if matches!(
        text,
        "Loading composer repositories with package information"
            | "Reading composer.json of"
            | "Reading /root/.cache/composer/repo"
    ) {
        return true;
    }
    let Some(operation) = text.strip_prefix("- ") else {
        return false;
    };
    let Some(package) = operation
        .strip_prefix("Downloading ")
        .or_else(|| operation.strip_prefix("Installing "))
    else {
        return false;
    };
    let Some((name, rest)) = package.split_once(' ') else {
        return false;
    };
    let mut parts = name.split('/');
    parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_none()
        && rest.starts_with('(')
        && rest.contains(')')
}
