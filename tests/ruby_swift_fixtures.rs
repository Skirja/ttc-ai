#![allow(dead_code)]
#[path = "../src/core/mod.rs"]
mod core;
#[path = "common/m8_support.rs"]
mod m8_support;

use core::classification::Family;
use m8_support::run_filter;

#[test]
fn ruby_test_and_lint_progress_reduce_with_diagnostics_retained() {
    for (family, record, passing) in [
        (Family::Rspec, "....\n", true),
        (Family::RakeTest, "....\n", true),
        (Family::Rubocop, "......\n", false),
    ] {
        let lines = (0..1001)
            .map(|_| ("stdout", record))
            .chain([("stdout", "1 example, 0 failures\n")])
            .chain([("stderr", "warning: preserve diagnostic\n")])
            .collect::<Vec<_>>();
        let result = run_filter(family, &lines);
        assert_eq!(
            result.report.passing,
            if passing { 1001 } else { 0 },
            "{family:?}"
        );
        assert_eq!(
            result.report.progress,
            if passing { 0 } else { 1001 },
            "{family:?}"
        );
        assert!(result.report.raw_id.is_some(), "{family:?}");
        assert_eq!(result.stderr, b"warning: preserve diagnostic\n");
        assert!(result.stdout.ends_with(b"1 example, 0 failures\n"));
        assert!(
            (result.stdout.len() + result.stderr.len() + result.metadata.len()) * 5
                < lines.iter().map(|(_, line)| line.len()).sum::<usize>()
        );
    }
}

#[test]
fn swift_build_and_test_records_reduce_but_failure_and_summary_remain() {
    for (family, record, passing) in [
        (
            Family::SwiftBuild,
            "Compile Swift Module 'Demo' (1 sources)\n",
            false,
        ),
        (
            Family::SwiftTest,
            "✔ Test DemoTests.testPass() passed after 0.01 seconds.\n",
            true,
        ),
    ] {
        let lines = (0..1001)
            .map(|_| ("stdout", record))
            .chain([("stdout", "Test Suite 'All tests' passed at 2026-01-01\n")])
            .chain([("stderr", "error: keep failure details\n")])
            .collect::<Vec<_>>();
        let result = run_filter(family, &lines);
        assert_eq!(
            result.report.passing,
            if passing { 998 } else { 0 },
            "{family:?}"
        );
        assert_eq!(
            result.report.progress,
            if passing { 0 } else { 998 },
            "{family:?}"
        );
        assert!(result.report.raw_id.is_some(), "{family:?}");
        assert_eq!(result.stderr, b"error: keep failure details\n");
        assert!(
            result
                .stdout
                .ends_with(b"Test Suite 'All tests' passed at 2026-01-01\n")
        );
        assert!(
            (result.stdout.len() + result.stderr.len() + result.metadata.len()) * 5
                < lines.iter().map(|(_, line)| line.len()).sum::<usize>()
        );
    }
}
