#![allow(dead_code)]

use crate::core::classification::{Family, Plan};
use crate::core::config::Config;
use crate::core::filters::AdditionalFilter;
use crate::core::raw_store::{StorePaths, Stream};
use crate::core::streaming::{self, Report};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("ttc-m8-filter-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create M8 fixture directory");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove M8 fixture directory");
    }
}

pub struct FilterRun {
    _dir: TempDir,
    pub report: Report,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub metadata: Vec<u8>,
}

pub fn run_filter(family: Family, lines: &[(&str, &str)]) -> FilterRun {
    let dir = TempDir::new();
    let (sender, receiver) = mpsc::channel();
    for (stream, line) in lines {
        let stream = if *stream == "stderr" {
            Stream::Stderr
        } else {
            Stream::Stdout
        };
        sender.send((stream, line.as_bytes().to_vec())).unwrap();
    }
    drop(sender);

    let paths = StorePaths([dir.0.join("state/runs"), dir.0.join("tmp/runs")]);
    let mut filter = AdditionalFilter::new(Plan {
        families: vec![family],
        ..Plan::default()
    });
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
    let mut metadata = Vec::new();
    streaming::write_metadata(&report, &mut metadata).unwrap();
    if std::env::var_os("TTC_M8_EVIDENCE").is_some() {
        let input_bytes = lines.iter().map(|(_, line)| line.len()).sum::<usize>();
        let output_bytes = stdout.len() + stderr.len() + metadata.len();
        eprintln!(
            "M8 fixture family={family:?} input_bytes={input_bytes} output_bytes={output_bytes} passing={} progress={}",
            report.passing, report.progress
        );
    }
    FilterRun {
        _dir: dir,
        report,
        stdout,
        stderr,
        metadata,
    }
}
