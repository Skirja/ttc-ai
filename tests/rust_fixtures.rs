#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, Plan};
use core::config::Config;
use core::filters::ecosystems::EcosystemFilter;
use core::raw_store::{StorePaths, Stream};
use core::streaming::{self, Filter};
use std::sync::mpsc;

fn filter(family: Family) -> EcosystemFilter {
    EcosystemFilter::new(Plan {
        families: vec![family],
        raw: false,
    })
}

#[test]
fn rust_recognizers_retain_failures_diagnostics_warnings_and_summary() {
    for (family, passing) in [
        (Family::RustTest, "test alpha::works ... ok\n"),
        (Family::RustNextest, "PASS [  0.001s] crate::alpha\n"),
        (Family::RustBuild, "Compiling crate v1.2.3 (/work/crate)\n"),
        (Family::RustCheck, "Checking crate v1.2.3 (/work/crate)\n"),
        (Family::RustClippy, "Checking crate v1.2.3 (/work/crate)\n"),
        (Family::RustDoc, "Documenting crate v1.2.3 (/work/crate)\n"),
    ] {
        let mut recognizer = filter(family);
        for _ in 0..4 {
            recognizer
                .decide(Stream::Stdout, passing.as_bytes())
                .unwrap();
        }
        for retained in [
            "test alpha::broken ... FAILED\n",
            "warning: deprecated item\n",
            "error[E0308]: mismatched types\n",
            "  --> src/lib.rs:10:5\n",
            "test result: FAILED. 1 failed\n",
            "application output\n",
        ] {
            assert!(
                recognizer
                    .decide(Stream::Stdout, retained.as_bytes())
                    .unwrap()
                    .is_none(),
                "{retained}"
            );
        }
    }
}

#[test]
fn large_libtest_fixture_reduces_by_at_least_eighty_percent() {
    let dir = TestDir::new();
    let input = (0..1_001)
        .map(|index| format!("test fixture::case_{index:04} ... ok\n"))
        .collect::<String>();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender
        .send((Stream::Stdout, input.as_bytes().to_vec()))
        .unwrap();
    sender
        .send((
            Stream::Stdout,
            b"test result: ok. 1001 passed; 0 failed\n".to_vec(),
        ))
        .unwrap();
    drop(sender);
    let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths),
        &mut filter(Family::RustTest),
        &mut stdout,
        &mut stderr,
    );
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    assert_eq!(report.passing, 998);
    assert!(report.raw_id.is_some());
    assert!((stdout.len() + stderr.len() + metadata.len()) * 5 < input.len());
    assert!(
        String::from_utf8(stdout)
            .unwrap()
            .contains("test result: ok. 1001 passed")
    );
}

#[test]
fn rust_parser_uses_ansi_for_classification_and_keeps_summaries() {
    let mut recognizer = filter(Family::RustTest);
    assert!(
        recognizer
            .decide(Stream::Stdout, b"\x1b[31mtest x ... ok\n")
            .unwrap()
            .is_none()
    );
    let mut recognizer = filter(Family::RustTest);
    for _ in 0..3 {
        assert!(
            recognizer
                .decide(Stream::Stdout, b"test x ... ok\r\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(
        recognizer
            .decide(Stream::Stdout, b"test x ... ok\r\n")
            .unwrap()
            .is_some()
    );
    assert!(
        recognizer
            .decide(Stream::Stdout, b"test result: ok. 4 passed\n")
            .unwrap()
            .is_none()
    );
}

#[test]
fn parser_confidence_is_independent_when_plans_contain_multiple_ecosystems() {
    let mut recognizer = EcosystemFilter::new(Plan {
        families: vec![Family::RustTest, Family::PyTest],
        raw: false,
    });
    for line in ["test rust::one ... ok\n", "test rust::two ... ok\n"] {
        assert!(
            recognizer
                .decide(Stream::Stdout, line.as_bytes())
                .unwrap()
                .is_none()
        );
    }
    for index in 0..3 {
        assert!(
            recognizer
                .decide(
                    Stream::Stdout,
                    format!("tests/test_python.py::test_{index} PASSED\n").as_bytes()
                )
                .unwrap()
                .is_none()
        );
    }
    assert!(
        recognizer
            .decide(Stream::Stdout, b"tests/test_python.py::test_3 PASSED\n")
            .unwrap()
            .is_some()
    );
}
