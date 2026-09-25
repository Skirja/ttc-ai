#![allow(dead_code)]
#[path = "../src/core/mod.rs"]
mod core;

use core::classification::{Family, Plan};
use core::filters::JsFilter;
use core::raw_store::Stream;
use core::streaming::{CompactKind, Filter};

fn make_filter(family: Family) -> JsFilter {
    JsFilter::new(Plan {
        families: vec![family],
        raw: false,
    })
}

#[test]
fn confidence_keeps_the_first_three_records() {
    let mut filter = make_filter(Family::Test);
    for _ in 0..3 {
        assert!(
            filter
                .decide(Stream::Stdout, "✓ safe test\n".as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter.decide(Stream::Stdout, "✓ safe test\n".as_bytes()),
        Ok(Some(CompactKind::Passing))
    ));
}

#[test]
fn unrelated_record_resets_output_signature_confidence() {
    let mut filter = make_filter(Family::Test);
    for _ in 0..3 {
        assert!(
            filter
                .decide(Stream::Stdout, b"PASS tests/a.test.js\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(
        filter
            .decide(Stream::Stdout, b"unrelated output\n")
            .unwrap()
            .is_none()
    );
    for _ in 0..3 {
        assert!(
            filter
                .decide(Stream::Stdout, b"PASS tests/b.test.js\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter.decide(Stream::Stdout, b"PASS tests/c.test.js\n"),
        Ok(Some(CompactKind::Passing))
    ));
}

#[test]
fn stderr_warning_does_not_block_stdout_passing_records() {
    let mut filter = make_filter(Family::Test);
    assert!(
        filter
            .decide(Stream::Stderr, b"warning: keep this\n")
            .unwrap()
            .is_none()
    );
    for _ in 0..3 {
        assert!(
            filter
                .decide(Stream::Stdout, b"PASS tests/a.test.js\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter.decide(Stream::Stdout, b"PASS tests/b.test.js\n"),
        Ok(Some(CompactKind::Passing))
    ));
}

#[test]
fn warnings_failures_diffs_locations_and_summaries_are_retained() {
    for line in [
        "✓ warning about test\n",
        "✓ error in test\n",
        "✓ failed test\n",
        "✓ panic happened\n",
        "✓ fatal issue\n",
        "✓ assert changed\n",
        "✓ expected actual\n",
        "✓ deprecated API\n",
        "✓ vulnerability security\n",
        "✓ snapshot diff\n",
        "✓ tests/a.test.ts:4:2\n",
        "✓ Makefile:42\n",
        "✓ README:3:1\n",
        "Tests: 5 passed\n",
        "✓ 5 tests passed\n",
        "    at function (file.js:2:4)\n",
        "unknown format\n",
    ] {
        let mut filter = make_filter(Family::Test);
        for _ in 0..4 {
            assert!(
                filter
                    .decide(Stream::Stderr, line.as_bytes())
                    .unwrap()
                    .is_none(),
                "{line}"
            );
        }
    }
}

#[test]
fn ansi_is_only_used_for_classification_and_structured_output_disables_filter() {
    let mut filter = make_filter(Family::Test);
    for _ in 0..3 {
        assert!(
            filter
                .decide(Stream::Stdout, b"\x1b[32mPASS tests/suite.test.js\x1b[0m\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        filter.decide(Stream::Stdout, b"\x1b[32mPASS tests/suite.test.js\x1b[0m\n"),
        Ok(Some(CompactKind::Passing))
    ));
    for prefix in [
        "{\"tests\":1}\n",
        "TAP version 14\n",
        "<?xml version=\"1.0\"?>\n",
        "---\n",
        "items: 12\n",
        "[\n",
    ] {
        let mut filter = make_filter(Family::Test);
        assert!(
            filter
                .decide(Stream::Stdout, prefix.as_bytes())
                .unwrap()
                .is_none()
        );
        for _ in 0..8 {
            assert!(
                filter
                    .decide(Stream::Stdout, b"PASS tests/suite.test.js\n")
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn malformed_ansi_fails_open() {
    let mut filter = make_filter(Family::Test);
    assert!(
        filter
            .decide(Stream::Stdout, b"\x1b[2KPASS tests/a.test.js\n")
            .is_err()
    );
}

#[test]
fn progress_families_accept_only_their_own_patterns() {
    let cases = [
        (Family::Lint, "[eslint] Checking src/a.js\n"),
        (Family::Typecheck, "[tsc] Checking src/a.ts\n"),
        (Family::Build, "transforming (42)\n"),
        (Family::Format, "[prettier] Checking src/a.js\n"),
        (
            Family::Install,
            "Progress: resolved 5, reused 4, downloaded 1\n",
        ),
    ];
    for (family, line) in cases {
        let mut filter = make_filter(family);
        for _ in 0..3 {
            assert!(
                filter
                    .decide(Stream::Stdout, line.as_bytes())
                    .unwrap()
                    .is_none()
            );
        }
        assert!(matches!(
            filter.decide(Stream::Stdout, line.as_bytes()),
            Ok(Some(CompactKind::Progress))
        ));
        assert!(
            filter
                .decide(Stream::Stdout, b"unknown progress\n")
                .unwrap()
                .is_none()
        );
    }
}
