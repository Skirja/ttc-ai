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
fn junit_pass_maven_download_and_gradle_task_progress_are_compacted_safely() {
    let mut junit = filter(Family::JvmTest);
    for _ in 0..3 {
        assert!(
            junit
                .decide(Stream::Stdout, "│  ├─ testPass() ✔\n".as_bytes())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        junit.decide(Stream::Stdout, "│  ├─ testPass() ✔\n".as_bytes()),
        Ok(Some(CompactKind::Passing))
    ));
    for line in [
        "✘ testFails()",
        "warning: deprecation",
        "expected true but was false",
        "BUILD SUCCESSFUL",
        "generic Java app",
    ] {
        assert!(
            junit
                .decide(Stream::Stdout, format!("{line}\n").as_bytes())
                .unwrap()
                .is_none()
        );
    }

    let mut maven = filter(Family::JvmProgress);
    for _ in 0..3 {
        maven
            .decide(
                Stream::Stdout,
                b"[INFO] Downloading from central: https://repo.test/pkg.jar\n",
            )
            .unwrap();
    }
    assert!(matches!(
        maven.decide(
            Stream::Stdout,
            b"[INFO] Downloading from central: https://repo.test/pkg.jar\n"
        ),
        Ok(Some(CompactKind::Progress))
    ));
    let mut maven_complete = filter(Family::JvmProgress);
    for _ in 0..3 {
        maven_complete
            .decide(
                Stream::Stdout,
                b"[INFO] Downloaded from central: https://repo.test/pkg.jar (1 kB at 10 kB/s)\n",
            )
            .unwrap();
    }
    assert!(matches!(
        maven_complete.decide(
            Stream::Stdout,
            b"[INFO] Downloaded from central: https://repo.test/pkg.jar (1 kB at 10 kB/s)\n"
        ),
        Ok(Some(CompactKind::Progress))
    ));

    let mut gradle = filter(Family::JvmBuild);
    for _ in 0..3 {
        gradle
            .decide(Stream::Stdout, b"> Task :module:test\n")
            .unwrap();
    }
    assert!(matches!(
        gradle.decide(Stream::Stdout, b"> Task :module:test\n"),
        Ok(Some(CompactKind::Progress))
    ));
    assert!(
        gradle
            .decide(Stream::Stdout, b"> Task :module:test FAILED\n")
            .unwrap()
            .is_none()
    );
    for status in ["UP-TO-DATE", "FROM-CACHE", "SKIPPED", "NO-SOURCE"] {
        let mut gradle_status = filter(Family::JvmBuild);
        let line = format!("> Task :module:test {status}\n");
        for _ in 0..3 {
            assert!(
                gradle_status
                    .decide(Stream::Stdout, line.as_bytes())
                    .unwrap()
                    .is_none()
            );
        }
        assert!(matches!(
            gradle_status.decide(Stream::Stdout, line.as_bytes()),
            Ok(Some(CompactKind::Progress))
        ));
    }
    let mut gradle_unknown_status = filter(Family::JvmBuild);
    for _ in 0..8 {
        assert!(
            gradle_unknown_status
                .decide(
                    Stream::Stdout,
                    b"> Task :module:test UNKNOWN-STATUS details\n"
                )
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn large_junit_fixture_reduces_with_summary_and_raw_capture() {
    let dir = TestDir::new();
    let input = "│  ├─ testWithLongStableName() ✔\n".repeat(1001);
    let (sender, receiver) = mpsc::sync_channel(2);
    sender
        .send((Stream::Stdout, input.as_bytes().to_vec()))
        .unwrap();
    sender
        .send((
            Stream::Stdout,
            b"[         1 tests successful      ]\n".to_vec(),
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
        &mut filter(Family::JvmTest),
        &mut stdout,
        &mut stderr,
    );
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    assert_eq!(report.passing, 998);
    assert!(report.raw_id.is_some());
    assert!(String::from_utf8_lossy(&stdout).contains("tests successful"));
    assert!((stdout.len() + stderr.len() + metadata.len()) * 5 < input.len());
}

#[test]
fn large_maven_and_gradle_progress_fixtures_include_ttc_metadata_in_reduction() {
    for (family, record) in [
        (
            Family::JvmProgress,
            "[INFO] Downloading from central: https://repo.test/artifact.jar\n",
        ),
        (Family::JvmBuild, "> Task :module:test\n"),
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
