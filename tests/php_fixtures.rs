#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, Plan};
use core::config::Config;
use core::filters::AdditionalFilter;
use core::raw_store::{StorePaths, Stream};
use core::streaming::{self, CompactKind, Filter};
use std::sync::mpsc;

fn filter(family: Family) -> AdditionalFilter {
    AdditionalFilter::new(Plan {
        families: vec![family],
        ..Plan::default()
    })
}

#[test]
fn phpunit_and_pest_progress_compacts_only_after_confidence() {
    for family in [Family::PhpTest] {
        let mut recognizer = filter(family);
        for _ in 0..3 {
            assert!(
                recognizer
                    .decide(Stream::Stdout, b"....\n")
                    .unwrap()
                    .is_none()
            );
        }
        assert!(matches!(
            recognizer.decide(Stream::Stdout, b"....\n"),
            Ok(Some(CompactKind::Passing))
        ));
        for line in [
            b"warning: deprecated test API\n".as_slice(),
            b"There was 1 failure:\n",
            b"Error: expected value, actual null\n",
            b"application output\n",
            b"{\"event\":\"pass\"}\n",
        ] {
            assert!(recognizer.decide(Stream::Stdout, line).unwrap().is_none());
        }
    }
    let mut progress = filter(Family::PhpTest);
    for _ in 0..3 {
        assert!(progress
            .decide(
                Stream::Stdout,
                b".............................................................   61 / 1001 (  6%)\n",
            )
            .unwrap()
            .is_none());
    }
    assert!(matches!(
        progress.decide(
            Stream::Stdout,
            b".............................................................   61 / 1001 (  6%)\n",
        ),
        Ok(Some(CompactKind::Passing))
    ));
    assert!(
        progress
            .decide(
                Stream::Stdout,
                b"......................................................F 61 / 1001 (  6%)\n",
            )
            .unwrap()
            .is_none()
    );
    let mut recognizer = filter(Family::PhpTest);
    assert!(
        recognizer
            .decide(Stream::Stdout, "✓ creates a user\n".as_bytes())
            .unwrap()
            .is_none()
    );
    for _ in 0..3 {
        recognizer
            .decide(Stream::Stdout, "✓ creates a user\n".as_bytes())
            .unwrap();
    }
    assert!(matches!(
        recognizer.decide(Stream::Stdout, "✓ creates a user\n".as_bytes()),
        Ok(Some(CompactKind::Passing))
    ));

    let mut testdox = filter(Family::PhpTest);
    for _ in 0..4 {
        testdox
            .decide(Stream::Stdout, "✔ creates a user\n".as_bytes())
            .unwrap();
    }
    assert!(
        testdox
            .decide(Stream::Stdout, "✘ creates a broken user\n".as_bytes())
            .unwrap()
            .is_none()
    );
    assert!(
        testdox
            .decide(Stream::Stdout, "✔ creates another user\n".as_bytes())
            .unwrap()
            .is_none()
    );

    let mut phpunit_header = filter(Family::PhpTest);
    for header in [
        b"Runtime: PHP 8.4.25\n".as_slice(),
        b"Configuration: /tmp/phpunit.xml\n",
        b"Time: 00:00.283, Memory: 24.00 MB\n",
    ] {
        assert!(
            phpunit_header
                .decide(Stream::Stdout, header)
                .unwrap()
                .is_none()
        );
    }
    for _ in 0..3 {
        phpunit_header
            .decide(Stream::Stdout, "✔ creates a user\n".as_bytes())
            .unwrap();
    }
    assert!(matches!(
        phpunit_header.decide(Stream::Stdout, "✔ creates a user\n".as_bytes()),
        Ok(Some(CompactKind::Passing))
    ));
}

#[test]
fn php_static_analysis_and_composer_recognize_only_known_progress() {
    let mut phpcs = filter(Family::PhpLint);
    for _ in 0..3 {
        assert!(
            phpcs
                .decide(Stream::Stdout, b".... 4/4\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        phpcs.decide(Stream::Stdout, b".... 4/4\n"),
        Ok(Some(CompactKind::Progress))
    ));
    assert!(
        phpcs
            .decide(Stream::Stdout, b"FILE: src/Foo.php\n")
            .unwrap()
            .is_none()
    );

    let mut composer = filter(Family::PhpInstall);
    for _ in 0..3 {
        assert!(
            composer
                .decide(Stream::Stdout, b"  - Downloading vendor/package (1.2.3)\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        composer.decide(Stream::Stdout, b"  - Downloading vendor/package (1.2.3)\n"),
        Ok(Some(CompactKind::Progress))
    ));
    assert!(
        composer
            .decide(Stream::Stdout, b"Package operations: 12 installs\n")
            .unwrap()
            .is_none()
    );
}

#[test]
fn large_phpunit_fixture_reduces_by_at_least_eighty_percent_with_raw_capture() {
    let dir = TestDir::new();
    let input = b"....\n".repeat(1001);
    let (sender, receiver) = mpsc::sync_channel(2);
    sender.send((Stream::Stdout, input.clone())).unwrap();
    drop(sender);
    let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths),
        &mut filter(Family::PhpTest),
        &mut stdout,
        &mut stderr,
    );
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    assert_eq!(report.passing, 998);
    assert!(report.raw_id.is_some());
    assert!((stdout.len() + stderr.len() + metadata.len()) * 5 < input.len());
}

#[test]
fn large_phpcs_and_composer_progress_fixtures_include_ttc_metadata_in_reduction() {
    for (family, record) in [
        (Family::PhpLint, "[1/1001]\n"),
        (
            Family::PhpInstall,
            "  - Downloading vendor/package (1.2.3)\n",
        ),
    ] {
        let dir = TestDir::new();
        let input = record.repeat(1001).into_bytes();
        let (sender, receiver) = mpsc::sync_channel(2);
        sender.send((Stream::Stdout, input.clone())).unwrap();
        drop(sender);
        let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let report = streaming::process_to(
            receiver,
            Config::default(),
            Some(paths),
            &mut filter(family),
            &mut stdout,
            &mut stderr,
        );
        let mut metadata = Vec::new();
        streaming::write_metadata(&report, &mut metadata).unwrap();
        assert_eq!(report.progress, 998, "{family:?}");
        assert!(report.raw_id.is_some(), "{family:?}");
        assert!(
            (stdout.len() + stderr.len() + metadata.len()) * 5 < input.len(),
            "{family:?}: {} / {}",
            stdout.len() + stderr.len() + metadata.len(),
            input.len()
        );
    }
}
