//! Conservative recognizers for build tools, Ruby, Swift, containers, and
//! infrastructure validation. Unmatched output is retained byte-for-byte.

use super::super::classification::Family;
use super::super::streaming::CompactKind;

pub(super) fn recognize(family: &Family, text: &str) -> Option<CompactKind> {
    let (kind, matched) = match family {
        Family::CmakeBuild => (
            CompactKind::Progress,
            cmake_progress(text) || ninja_progress(text),
        ),
        Family::NinjaBuild => (CompactKind::Progress, ninja_progress(text)),
        Family::MakeBuild => (
            CompactKind::Progress,
            cmake_progress(text) || ninja_progress(text) || ctest_pass(text),
        ),
        Family::Ctest => (CompactKind::Passing, ctest_pass(text)),
        Family::Rspec | Family::RakeTest => (CompactKind::Passing, passing_dots(text)),
        Family::Rubocop => (CompactKind::Progress, passing_dots(text)),
        Family::SwiftBuild => (CompactKind::Progress, swift_build_progress(text)),
        Family::SwiftTest => (CompactKind::Passing, swift_test_pass(text)),
        Family::ContainerBuild => (CompactKind::Progress, buildkit_load_progress(text)),
        Family::HelmLint => (CompactKind::Progress, helm_chart_progress(text)),
        // terraform validate output is diagnostic and summary content. The
        // family is classified deliberately, with no compactable records.
        Family::TerraformValidate => return None,
        _ => return None,
    };
    matched.then_some(kind)
}

fn cmake_progress(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('[') else {
        return false;
    };
    let Some((percent, action)) = rest.split_once("] ") else {
        return false;
    };
    let Some(value) = percent.trim().strip_suffix('%') else {
        return false;
    };
    value.parse::<u8>().is_ok_and(|value| value <= 100)
        && [
            "Building C object ",
            "Building CXX object ",
            "Building ASM object ",
            "Linking C executable ",
            "Linking CXX executable ",
            "Linking C static library ",
            "Linking CXX static library ",
        ]
        .iter()
        .any(|prefix| action.starts_with(prefix) && action.len() <= 2048)
}

fn ninja_progress(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('[') else {
        return false;
    };
    let Some((counter, action)) = rest.split_once("] ") else {
        return false;
    };
    let Some((current, total)) = counter.split_once('/') else {
        return false;
    };
    current.parse::<u32>().is_ok_and(|n| n > 0)
        && total.parse::<u32>().is_ok_and(|n| n > 0)
        && [
            "Building C object ",
            "Building CXX object ",
            "Building ASM object ",
            "Linking C executable ",
            "Linking CXX executable ",
            "Linking C static library ",
            "Linking CXX static library ",
        ]
        .iter()
        .any(|prefix| action.starts_with(prefix) && action.len() <= 2048)
}

fn ctest_pass(text: &str) -> bool {
    let text = text.trim();
    let Some((prefix, result)) = text.split_once(" ... Passed ") else {
        return false;
    };
    let Some((progress, name)) = prefix.split_once(" Test #") else {
        return false;
    };
    let Some((index, total)) = progress.split_once('/') else {
        return false;
    };
    let Some((number, name)) = name.split_once(':') else {
        return false;
    };
    let Some((seconds, unit)) = result.split_once(' ') else {
        return false;
    };
    index.parse::<u32>().is_ok()
        && total.parse::<u32>().is_ok()
        && number.parse::<u32>().is_ok()
        && !name.trim().is_empty()
        && seconds.parse::<f64>().is_ok()
        && unit == "sec"
}

fn passing_dots(text: &str) -> bool {
    !text.is_empty() && text.len() <= 1024 && text.bytes().all(|byte| byte == b'.')
}

fn swift_build_progress(text: &str) -> bool {
    [
        "Compile Swift ",
        "Compile Swift Module ",
        "Emitting module ",
        "Linking ",
    ]
    .iter()
    .any(|prefix| text.starts_with(prefix) && text.len() <= 2048)
}

fn swift_test_pass(text: &str) -> bool {
    if let Some(rest) = text.strip_prefix("Test Case '")
        && let Some((name, duration)) = rest.rsplit_once("' passed (")
        && let Some(seconds) = duration.strip_suffix(" seconds).")
    {
        return !name.is_empty() && seconds.parse::<f64>().is_ok();
    }

    let Some(rest) = text.strip_prefix("✔ Test ") else {
        return false;
    };
    let Some((name, duration)) = rest.split_once(" passed after ") else {
        return false;
    };
    let Some(seconds) = duration.strip_suffix(" seconds.") else {
        return false;
    };
    !name.is_empty() && seconds.parse::<f64>().is_ok()
}

fn buildkit_load_progress(text: &str) -> bool {
    let Some((id, description)) = text.split_once(" [internal] ") else {
        return false;
    };
    let Some(number) = id.strip_prefix('#') else {
        return false;
    };
    number.parse::<u32>().is_ok()
        && [
            "load build definition from ",
            "load .dockerignore",
            "load build context",
            "load metadata for ",
        ]
        .iter()
        .any(|prefix| description.starts_with(prefix) && description.len() <= 2048)
}

fn helm_chart_progress(text: &str) -> bool {
    text.strip_prefix("==> Linting ")
        .is_some_and(|chart| !chart.is_empty() && chart.len() <= 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_known_progress_and_passing_grammars() {
        assert!(cmake_progress("[ 25%] Building CXX object CMakeFiles/x.o"));
        assert!(ninja_progress("[2/4] Linking CXX executable app"));
        assert!(ctest_pass(" 1/2 Test #1: smoke ... Passed 0.01 sec"));
        assert!(swift_test_pass(
            "Test Case '-[Tests.Core testPass]' passed (0.1 seconds)."
        ));
        assert!(swift_test_pass(
            "✔ Test Core.testPass() passed after 0.1 seconds."
        ));
        assert!(buildkit_load_progress(
            "#1 [internal] load build definition from Dockerfile"
        ));
        assert!(helm_chart_progress("==> Linting charts/demo"));
    }

    #[test]
    fn keeps_failure_summary_custom_targets_and_application_steps() {
        for line in [
            "[100%] Built target app",
            "[1/2] Generating version.rs",
            " 2/2 Test #2: smoke ...***Failed 0.01 sec",
            "Test Case '-[Tests.Core testFail]' failed (0.1 seconds).",
            "#2 [build 1/2] RUN npm install",
            "Error: chart has an invalid template",
            "1 file inspected, no offenses detected",
        ] {
            assert!(!cmake_progress(line), "{line}");
            assert!(!ninja_progress(line), "{line}");
            assert!(!ctest_pass(line), "{line}");
            assert!(!buildkit_load_progress(line), "{line}");
            assert!(!helm_chart_progress(line), "{line}");
        }
        assert!(!passing_dots("...F......"));
        assert!(!swift_test_pass("✔ Test Core.testPass() passed."));
    }
}
