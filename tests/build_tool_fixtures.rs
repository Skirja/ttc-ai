#![allow(dead_code)]
#[path = "../src/core/mod.rs"]
mod core;
#[path = "common/m8_support.rs"]
mod m8_support;

use core::classification::Family;
use m8_support::run_filter;

#[test]
fn build_tool_families_compact_only_repeated_known_records() {
    for (family, record, passing) in [
        (
            Family::CmakeBuild,
            "[ 25%] Building CXX object CMakeFiles/demo.dir/main.cpp.o\n",
            false,
        ),
        (
            Family::NinjaBuild,
            "[2/4] Linking CXX executable demo\n",
            false,
        ),
        (
            Family::MakeBuild,
            "1/2 Test #1: smoke ............................   Passed    0.01 sec\n",
            false,
        ),
        (
            Family::Ctest,
            "1/2 Test #1: smoke ............................   Passed    0.01 sec\n",
            true,
        ),
    ] {
        let lines = (0..1001)
            .map(|_| ("stdout", record))
            .chain([("stdout", "100% tests passed, 0 tests failed out of 2\n")])
            .chain([("stderr", "warning: keep this warning\n")])
            .collect::<Vec<_>>();
        let result = run_filter(family, &lines);
        assert_eq!(
            result.report.progress,
            if passing { 0 } else { 998 },
            "{family:?}"
        );
        assert_eq!(
            result.report.passing,
            if passing { 998 } else { 0 },
            "{family:?}"
        );
        assert!(result.report.raw_id.is_some(), "{family:?}");
        assert!(
            result
                .stdout
                .ends_with(b"100% tests passed, 0 tests failed out of 2\n")
        );
        assert_eq!(result.stderr, b"warning: keep this warning\n");
        assert!(
            (result.stdout.len() + result.stderr.len() + result.metadata.len()) * 5
                < lines.iter().map(|(_, line)| line.len()).sum::<usize>()
        );
    }
}

#[test]
fn four_step_ninja_build_with_a_warning_stays_byte_exact() {
    for family in [Family::CmakeBuild, Family::NinjaBuild] {
        for warning_after in 1..=3 {
            let mut lines = vec![
                (
                    "stdout",
                    "[1/4] Building CXX object CMakeFiles/demo.dir/main.cpp.o\n",
                ),
                ("stdout", "[2/4] Linking CXX executable demo-cxx\n"),
                (
                    "stdout",
                    "[3/4] Building C object CMakeFiles/demo.dir/main.c.o\n",
                ),
                ("stdout", "[4/4] Linking C executable demo\n"),
            ];
            lines.insert(
                warning_after,
                (
                    "stdout",
                    "main.c:2:2: warning: TTC M8 build warning retention\n",
                ),
            );
            let result = run_filter(family, &lines);
            let original = lines
                .iter()
                .flat_map(|(_, line)| line.bytes())
                .collect::<Vec<_>>();
            assert_eq!(
                result.stdout, original,
                "{family:?} warning after {warning_after}"
            );
            assert!(result.stderr.is_empty());
            assert!(result.metadata.is_empty());
            assert_eq!(result.report.progress, 0);
        }
    }
}

#[test]
fn one_failure_or_custom_progress_line_never_compacts() {
    for (family, line) in [
        (Family::CmakeBuild, "[100%] Built target demo\n"),
        (Family::NinjaBuild, "[1/2] Generating generated.rs\n"),
        (Family::Ctest, "1/1 Test #1: smoke ...***Failed 0.01 sec\n"),
        (Family::MakeBuild, "make: *** [test] Error 1\n"),
    ] {
        let result = run_filter(family, &[("stdout", line)]);
        assert_eq!(result.stdout, line.as_bytes(), "{family:?}");
        assert!(result.report.raw_id.is_none(), "{family:?}");
        assert!(result.metadata.is_empty(), "{family:?}");
    }
}
