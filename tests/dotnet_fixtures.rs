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
fn dotnet_test_restore_and_build_progress_keep_failures_warnings_and_summaries() {
    let mut tests = filter(Family::DotnetTest);
    for _ in 0..3 {
        assert!(
            tests
                .decide(
                    Stream::Stdout,
                    b"Passed Example.CalculatorTests.Add [4 ms]\r\n"
                )
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        tests.decide(
            Stream::Stdout,
            b"Passed Example.CalculatorTests.Add [4 ms]\r\n"
        ),
        Ok(Some(CompactKind::Passing))
    ));
    let mut under_one_ms = filter(Family::DotnetTest);
    for _ in 0..3 {
        assert!(
            under_one_ms
                .decide(Stream::Stdout, b"Passed AddsAnInteger (1) [< 1 ms]\n")
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        under_one_ms.decide(Stream::Stdout, b"Passed AddsAnInteger (1) [< 1 ms]\n"),
        Ok(Some(CompactKind::Passing))
    ));
    let mut with_build_header = filter(Family::DotnetTest);
    assert!(
        with_build_header
            .decide(Stream::Stdout, b"Build started 09/29/2026 10:07:47.\n")
            .unwrap()
            .is_none()
    );
    assert!(
        with_build_header
            .decide(
                Stream::Stdout,
                b"Test run for /tmp/Tests.dll (.NETCoreApp,Version=v10.0)\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(
        with_build_header
            .decide(
                Stream::Stdout,
                b"A total of 1 test files matched the specified pattern.\n"
            )
            .unwrap()
            .is_none()
    );
    for _ in 0..3 {
        with_build_header
            .decide(Stream::Stdout, b"Passed Example.Tests.Add [4 ms]\n")
            .unwrap();
    }
    assert!(matches!(
        with_build_header.decide(Stream::Stdout, b"Passed Example.Tests.Add [4 ms]\n"),
        Ok(Some(CompactKind::Passing))
    ));
    for line in [
        "Failed Example.CalculatorTests.Divide [4 ms]",
        "warning CS0618: obsolete API",
        "error CS0103: name not found",
        "Expected: 1\n  Actual: 2",
        "Passed: 1, Failed: 0, Skipped: 0",
        "generic application output",
    ] {
        assert!(
            tests
                .decide(Stream::Stdout, format!("{line}\n").as_bytes())
                .unwrap()
                .is_none()
        );
    }

    let mut restore = filter(Family::DotnetRestore);
    for _ in 0..3 {
        restore
            .decide(Stream::Stdout, b"Determining projects to restore...\n")
            .unwrap();
    }
    assert!(matches!(
        restore.decide(Stream::Stdout, b"Determining projects to restore...\n"),
        Ok(Some(CompactKind::Progress))
    ));

    let mut build = filter(Family::DotnetBuild);
    for _ in 0..3 {
        build
            .decide(
                Stream::Stdout,
                b"Example -> /tmp/bin/Debug/net10.0/Example.dll\n",
            )
            .unwrap();
    }
    assert!(matches!(
        build.decide(
            Stream::Stdout,
            b"Example -> /tmp/bin/Debug/net10.0/Example.dll\n"
        ),
        Ok(Some(CompactKind::Progress))
    ));
}

#[test]
fn large_dotnet_test_fixture_reduces_and_retains_summary() {
    let dir = TestDir::new();
    let input = b"Passed Example.Tests.LongStableTestName [5 ms]\n".repeat(1001);
    let (sender, receiver) = mpsc::sync_channel(2);
    sender.send((Stream::Stdout, input.clone())).unwrap();
    sender
        .send((
            Stream::Stdout,
            b"Passed: 1001, Failed: 0, Skipped: 0\n".to_vec(),
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
        &mut filter(Family::DotnetTest),
        &mut stdout,
        &mut stderr,
    );
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    assert_eq!(report.passing, 998);
    assert!(report.raw_id.is_some());
    assert!(String::from_utf8_lossy(&stdout).contains("Passed: 1001"));
    assert!((stdout.len() + stderr.len() + metadata.len()) * 5 < input.len());
}

#[test]
fn large_dotnet_restore_and_build_progress_include_ttc_metadata_in_reduction() {
    for (family, record) in [
        (
            Family::DotnetRestore,
            "Determining projects to restore...\n",
        ),
        (
            Family::DotnetBuild,
            "Example -> /tmp/bin/Debug/net10.0/Example.dll\n",
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
