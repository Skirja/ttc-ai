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
fn python_recognizers_are_specific_and_retain_diagnostics_and_summaries() {
    for (family, passing) in [
        (Family::PyTest, "tests/test_math.py::test_add PASSED\n"),
        (
            Family::PyUnittest,
            "test_add (tests.test_math.TestMath) ... ok\n",
        ),
        (Family::PyInstall, "Collecting sample-package>=1\n"),
    ] {
        let mut recognizer = filter(family);
        for _ in 0..4 {
            recognizer
                .decide(Stream::Stdout, passing.as_bytes())
                .unwrap();
        }
        for retained in [
            "tests/test_math.py::test_bad FAILED\n",
            "WARNING: deprecated API\n",
            "E   AssertionError: expected 2 got 3\n",
            "coverage: 89%\n",
            "4 passed, 1 warning in 0.02s\n",
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
    let mut recognizer = filter(Family::PyTest);
    for _ in 0..3 {
        assert!(
            recognizer
                .decide(
                    Stream::Stdout,
                    b"tests/test_math.py::test_add PASSED                               [  0%]\n"
                )
                .unwrap()
                .is_none()
        );
    }
    assert!(
        recognizer
            .decide(
                Stream::Stdout,
                b"tests/test_math.py::test_add PASSED                               [100%]\n"
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn large_pytest_fixture_reduces_by_at_least_eighty_percent() {
    let dir = TestDir::new();
    let input = (0..1_001)
        .map(|index| format!("tests/test_many.py::test_case_{index:04} PASSED\n"))
        .collect::<String>();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender
        .send((Stream::Stdout, input.as_bytes().to_vec()))
        .unwrap();
    sender
        .send((Stream::Stdout, b"1001 passed in 0.40s\n".to_vec()))
        .unwrap();
    drop(sender);
    let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths),
        &mut filter(Family::PyTest),
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
            .contains("1001 passed in 0.40s")
    );
}

#[test]
fn python_install_and_test_edges_stay_visible() {
    let mut recognizer = filter(Family::PyInstall);
    for _ in 0..3 {
        assert!(
            recognizer
                .decide(Stream::Stdout, b"Downloading sample.whl\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(
        recognizer
            .decide(Stream::Stdout, b"Downloading sample.whl\n")
            .unwrap()
            .is_some()
    );
    assert!(
        recognizer
            .decide(Stream::Stderr, b"WARNING: insecure package index\n")
            .unwrap()
            .is_none()
    );
    assert!(
        recognizer
            .decide(Stream::Stdout, b"Successfully installed sample\n")
            .unwrap()
            .is_none()
    );
    assert!(
        recognizer
            .decide(Stream::Stdout, b"Downloading malformed\n")
            .unwrap()
            .is_none()
    );
}

#[test]
fn diagnostic_continuation_that_looks_like_a_pass_remains_visible() {
    let mut recognizer = filter(Family::PyTest);
    assert!(
        recognizer
            .decide(Stream::Stderr, b"WARNING: retain this diagnostic\n")
            .unwrap()
            .is_none()
    );
    assert!(
        recognizer
            .decide(
                Stream::Stderr,
                b"tests/test_example.py::test_fake PASSED [100%]\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(recognizer.decide(Stream::Stderr, b"\n").unwrap().is_none());
    for _ in 0..3 {
        assert!(
            recognizer
                .decide(
                    Stream::Stdout,
                    b"tests/test_example.py::test_real PASSED [ 50%]\n"
                )
                .unwrap()
                .is_none()
        );
    }
    assert!(
        recognizer
            .decide(
                Stream::Stdout,
                b"tests/test_example.py::test_real PASSED [100%]\n"
            )
            .unwrap()
            .is_some()
    );
}
