#![allow(dead_code)]
mod common;
#[path = "../src/core/mod.rs"]
mod core;

use common::TestDir;
use core::classification::{Family, Plan};
use core::config::Config;
use core::filters::JsFilter;
use core::raw_store::{StorePaths, Stream};
use core::streaming::{self, Filter};
use std::sync::mpsc;

struct Case {
    family: Family,
    success: &'static str,
    failure: &'static str,
    warning: &'static str,
}

const CASES: &[Case] = &[
    Case {
        family: Family::Test,
        success: "✓ verifies output safely\n",
        failure: "✗ failed test: expected 2 actual 3\n",
        warning: "warning: test used deprecated API\n",
    },
    Case {
        family: Family::Lint,
        success: "[eslint] Checking src/index.js\n",
        failure: "src/index.js:2:1 error no-unused-vars\n",
        warning: "src/index.js:2:1 warning no-console\n",
    },
    Case {
        family: Family::Typecheck,
        success: "[tsc] Checking src/index.ts\n",
        failure: "src/index.ts:2:1 error TS2304\n",
        warning: "warning: deprecated type\n",
    },
    Case {
        family: Family::Build,
        success: "transforming (42)\n",
        failure: "src/index.js:2:1 error: failed to build\n",
        warning: "warning: deprecated dependency\n",
    },
    Case {
        family: Family::Format,
        success: "[prettier] Checking src/index.js\n",
        failure: "[warn] src/index.js\n",
        warning: "warning: ignored file\n",
    },
    Case {
        family: Family::Install,
        success: "Progress: resolved 5, reused 4, downloaded 1\n",
        failure: "error: network failure\n",
        warning: "warning: vulnerability found\n",
    },
];

fn filter(family: Family) -> JsFilter {
    JsFilter::new(Plan {
        families: vec![family],
        raw: false,
        ..Plan::default()
    })
}

#[test]
fn every_family_retains_failure_warning_unknown_and_summary() {
    for case in CASES {
        let dir = TestDir::new();
        let mut decision_filter = filter(case.family);
        let records = [
            case.success,
            case.failure,
            case.warning,
            "unknown format\n",
            "Summary: 1 completed\n",
        ];
        for record in records {
            assert!(
                decision_filter
                    .decide(Stream::Stdout, record.as_bytes())
                    .unwrap()
                    .is_none(),
                "{record}"
            );
        }
        let (sender, receiver) = mpsc::sync_channel(8);
        sender
            .send((Stream::Stdout, case.success.repeat(4).into_bytes()))
            .unwrap();
        sender
            .send((
                Stream::Stderr,
                [case.failure, case.warning].concat().into_bytes(),
            ))
            .unwrap();
        sender
            .send((
                Stream::Stdout,
                b"unknown format\nSummary: 1 completed\n".to_vec(),
            ))
            .unwrap();
        drop(sender);
        let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let report = streaming::process_to(
            receiver,
            Config::default(),
            Some(paths),
            &mut filter(case.family),
            &mut output,
            &mut errors,
        );
        assert_eq!(
            output,
            [
                case.success.repeat(3).as_bytes(),
                b"unknown format\nSummary: 1 completed\n"
            ]
            .concat()
        );
        assert_eq!(errors, [case.failure, case.warning].concat().as_bytes());
        assert_eq!(report.passing + report.progress, 1);
    }
}

#[test]
fn every_large_family_fixture_reduces_at_least_eighty_percent() {
    for case in CASES {
        let dir = TestDir::new();
        let mut filter = filter(case.family);
        let input = case.success.repeat(1_001).into_bytes();
        let (sender, receiver) = mpsc::sync_channel(8);
        sender.send((Stream::Stdout, input.clone())).unwrap();
        drop(sender);
        let paths = StorePaths([dir.path().join("state/runs"), dir.path().join("tmp/runs")]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let report = streaming::process_to(
            receiver,
            Config::default(),
            Some(paths),
            &mut filter,
            &mut stdout,
            &mut stderr,
        );
        let compacted = report.passing + report.progress;
        assert_eq!(compacted, 998);
        assert!(report.raw_id.is_some());
        let mut metadata = Vec::new();
        streaming::write_metadata(&report, &mut metadata).unwrap();
        let model_bytes = stdout.len() + stderr.len() + metadata.len();
        assert!(model_bytes * 5 < input.len());
        println!(
            "{:?}: input_bytes={} model_bytes={} compacted_records={}",
            case.family,
            input.len(),
            model_bytes,
            compacted
        );
        for record in [case.failure, case.warning, "Summary: 1 completed\n"] {
            assert!(
                filter
                    .decide(Stream::Stderr, record.as_bytes())
                    .unwrap()
                    .is_none()
            );
        }
    }
}
