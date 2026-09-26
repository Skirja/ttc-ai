#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, Plan};
use core::config::Config;
use core::filters::ecosystems::EcosystemFilter;
use core::raw_store::{self, Selection, StorePaths, Stream};
use core::streaming::{self, Filter};
use std::sync::mpsc;

fn filter(family: Family) -> EcosystemFilter {
    EcosystemFilter::new(Plan {
        families: vec![family],
        raw: false,
    })
}

fn pass_event(test: &str) -> String {
    format!(
        "{{\"Action\":\"output\",\"Package\":\"example/pkg\",\"Test\":\"{test}\",\"OutputType\":\"frame\",\"Output\":\"--- PASS: {test} (0.00s)\\n\"}}\n"
    )
}

#[test]
fn go_text_recognizers_keep_failures_build_warnings_panics_and_summaries() {
    let mut recognizer = filter(Family::GoTest);
    for _ in 0..4 {
        recognizer
            .decide(Stream::Stdout, b"--- PASS: TestAdd (0.00s)\n")
            .unwrap();
    }
    for retained in [
        "--- FAIL: TestBroken (0.00s)\n",
        "panic: unexpected state\n",
        "WARNING: DATA RACE\n",
        "# example/pkg\n./x.go:10:2: undefined: Missing\n",
        "FAIL\texample/pkg [build failed]\n",
        "ok\texample/pkg\t0.10s\n",
        "arbitrary application output\n",
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

#[test]
fn go_run_lifecycle_stays_visible_while_passing_confidence_accumulates() {
    let mut recognizer = filter(Family::GoTest);
    for index in 0..4 {
        let run = format!("=== RUN   TestCase{index}\n");
        assert!(
            recognizer
                .decide(Stream::Stdout, run.as_bytes())
                .unwrap()
                .is_none()
        );
        let passing = format!("--- PASS: TestCase{index} (0.00s)\n");
        let result = recognizer
            .decide(Stream::Stdout, passing.as_bytes())
            .unwrap();
        assert_eq!(result.is_some(), index == 3);
    }
}

#[test]
fn large_go_verbose_fixture_reduces_by_at_least_eighty_percent() {
    let dir = TestDir::new();
    let input = (0..1_001)
        .map(|index| format!("--- PASS: TestCase{index:04} (0.00s)\n"))
        .collect::<String>();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender
        .send((Stream::Stdout, input.as_bytes().to_vec()))
        .unwrap();
    sender
        .send((Stream::Stdout, b"ok\texample/pkg\t0.10s\n".to_vec()))
        .unwrap();
    drop(sender);
    let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths),
        &mut filter(Family::GoTest),
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
            .contains("ok\texample/pkg")
    );
}

#[test]
fn go_json_preserves_lifecycle_and_only_compacts_matching_pass_frames() {
    let mut recognizer = filter(Family::GoJson);
    let lifecycle = [
        b"{\"Action\":\"start\",\"Package\":\"example/pkg\"}\n".as_slice(),
        b"{\"Action\":\"run\",\"Package\":\"example/pkg\",\"Test\":\"TestOne\"}\n",
    ];
    for line in lifecycle {
        assert!(recognizer.decide(Stream::Stdout, line).unwrap().is_none());
    }
    assert!(
        recognizer
            .decide(Stream::Stdout, pass_event("TestOne").as_bytes())
            .unwrap()
            .is_none()
    );
    assert!(recognizer.decide(Stream::Stdout, b"{\"Action\":\"pass\",\"Package\":\"example/pkg\",\"Test\":\"TestOne\",\"Elapsed\":0}\n").unwrap().is_none());
    assert!(
        recognizer
            .decide(Stream::Stdout, pass_event("TestTwo").as_bytes())
            .unwrap()
            .is_some()
    );
    assert!(
        recognizer
            .decide(
                Stream::Stdout,
                b"{\"Action\":\"pass\",\"Package\":\"example/pkg\",\"Elapsed\":0.01}\n"
            )
            .unwrap()
            .is_none()
    );
    assert!(recognizer.decide(Stream::Stdout, b"{\"Action\":\"fail\",\"Package\":\"example/pkg\",\"FailedBuild\":\"example/pkg\"}\n").unwrap().is_none());
}

#[test]
fn go_json_retains_multiline_diagnostic_unknown_fields_and_ambiguous_json() {
    let mut recognizer = filter(Family::GoJson);
    let multiline = b"{\"Action\":\"output\",\"Package\":\"example/pkg\",\"Test\":\"TestBad\",\"OutputType\":\"error-continue\",\"Output\":\"    want 1\\n    got 2\\n\"}\n";
    assert!(
        recognizer
            .decide(Stream::Stdout, multiline)
            .unwrap()
            .is_none()
    );
    for malformed in [
        b"{\"Action\":\"output\",\"Action\":\"pass\"}\n".as_slice(),
        b"{\"Action\":\"new-action\"}\n",
        b"{\"Action\":\"pass\",\"NewField\":true}\n",
        b"{\"Action\":\"output\",\"Test\":3}\n",
        b"{\"Action\":\"output\"\n",
    ] {
        assert!(recognizer.decide(Stream::Stdout, malformed).is_err());
    }
}

#[test]
fn go_json_compaction_keeps_a_byte_exact_raw_replay() {
    let dir = TestDir::new();
    let mut input = String::from("{\"Action\":\"start\",\"Package\":\"example/pkg\"}\n");
    for index in 0..8 {
        input.push_str(&pass_event(&format!("Test{index}")));
        input.push_str(&format!("{{\"Action\":\"pass\", \"Package\":\"example/pkg\",\"Test\":\"Test{index}\",\"Elapsed\":0}}\n"));
    }
    let original = input.as_bytes().to_vec();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender.send((Stream::Stdout, original.clone())).unwrap();
    sender
        .send((Stream::Stderr, b"go test stderr\n".to_vec()))
        .unwrap();
    drop(sender);
    let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths.clone()),
        &mut filter(Family::GoJson),
        &mut stdout,
        &mut stderr,
    );
    assert!(report.passing > 0);
    assert_eq!(stderr, b"go test stderr\n");
    let mut expected_visible = String::from("{\"Action\":\"start\",\"Package\":\"example/pkg\"}\n");
    expected_visible.push_str(&pass_event("Test0"));
    for index in 0..8 {
        expected_visible.push_str(&format!("{{\"Action\":\"pass\", \"Package\":\"example/pkg\",\"Test\":\"Test{index}\",\"Elapsed\":0}}\n"));
    }
    assert_eq!(stdout, expected_visible.as_bytes());
    let id = report.raw_id.unwrap();
    let mut replay_stdout = Vec::new();
    let mut replay_stderr = Vec::new();
    raw_store::replay_at_to(
        &paths,
        &id,
        Selection::Both,
        None,
        &mut replay_stdout,
        &mut replay_stderr,
    )
    .unwrap();
    assert_eq!(replay_stdout, original);
    assert_eq!(replay_stderr, b"go test stderr\n");
}

#[test]
fn malformed_go_json_after_compaction_fails_open_and_keeps_capture_replayable() {
    let dir = TestDir::new();
    let start = b"{\"Action\":\"start\",\"Package\":\"example/pkg\"}\n";
    let first = pass_event("Test0");
    let first_status =
        b"{\"Action\":\"pass\",\"Package\":\"example/pkg\",\"Test\":\"Test0\",\"Elapsed\":0}\n";
    let second = pass_event("Test1");
    let malformed = b"{\"Action\":\"output\",\"Action\":\"pass\"}\n";
    let tail = b"{\"Action\":\"fail\",\"Package\":\"example/pkg\"}\n";
    let input = [
        start.as_slice(),
        first.as_bytes(),
        first_status,
        second.as_bytes(),
        malformed,
        tail,
    ]
    .concat();
    let (sender, receiver) = mpsc::sync_channel(8);
    sender.send((Stream::Stdout, input.clone())).unwrap();
    drop(sender);
    let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let report = streaming::process_to(
        receiver,
        Config::default(),
        Some(paths.clone()),
        &mut filter(Family::GoJson),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(report.passing, 1);
    assert!(stdout.ends_with(&[malformed.as_slice(), tail].concat()));
    let id = report.raw_id.unwrap();
    let mut replay = Vec::new();
    raw_store::replay_at_to(
        &paths,
        &id,
        Selection::Stdout,
        None,
        &mut replay,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(replay, input);
}
