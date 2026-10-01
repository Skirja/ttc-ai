mod common;

use common::TestDir;
use std::fs;
use std::process::Command;

fn assert_dot_meter(metadata: &str, direct: &str, retained: &str) -> bool {
    let dir = TestDir::new();
    fs::write(dir.path().join("meter-direct.out"), direct).unwrap();
    fs::write(dir.path().join("meter-ttc.out"), retained).unwrap();
    fs::write(dir.path().join("meter-ttc.err"), metadata).unwrap();
    let output = Command::new("sh")
        .args([
            "-eu",
            "-c",
            ". \"$1\"; scratch=$2; assert_dot_meter_compacted meter",
            "m8-smoke-regression",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/scripts/m8-smoke-assertions.sh"
            ),
        ])
        .arg(dir.path())
        .output()
        .unwrap();
    output.status.success()
}

#[test]
fn dot_meter_accepts_passing_progress_and_mixed_compaction_metadata() {
    for metadata in [
        "TTC: 1 passing records and 0 progress records compacted\n",
        "TTC: 0 passing records and 1 progress records compacted\n",
        "TTC: 2 passing records and 3 progress records compacted\nraw: ttc raw abc\n",
    ] {
        assert!(
            assert_dot_meter(metadata, "....\n", "summary\n"),
            "{metadata}"
        );
    }
}

#[test]
fn dot_meter_rejects_zero_missing_or_malformed_compaction_metadata() {
    for metadata in [
        "TTC: 0 passing records and 0 progress records compacted\n",
        "",
        "TTC: 1 passing records and 0 progress records\ncompacted\n",
        "TTC: 0 passing records and -1 progress records compacted\n",
        "diagnostic: TTC: 0 passing records and 1 progress records compacted\n",
    ] {
        assert!(
            !assert_dot_meter(metadata, "....\n", "summary\n"),
            "{metadata}"
        );
    }
}

#[test]
fn dot_meter_rejects_missing_direct_meter_and_retained_ttc_meter() {
    let metadata = "TTC: 0 passing records and 1 progress records compacted\n";
    assert!(!assert_dot_meter(metadata, "summary\n", "summary\n"));
    assert!(!assert_dot_meter(metadata, "....\n", "....\nsummary\n"));
}

#[test]
fn smoke_capture_preserves_streams_status_and_count_with_shell_trace_enabled() {
    for trace in [false, true] {
        let dir = TestDir::new();
        let output = Command::new("sh")
            .args([
                if trace { "-eux" } else { "-eu" },
                "-c",
                ". \"$1\"; m8_capture_in \"$2\" \"$2/capture\" \"$2/cache\" sh -c 'printf x >> invocation.count; printf \"stdout\\000exact\\n\"; printf \"warning: stderr exact\\n\" >&2; exit 7'",
                "m8-capture-regression",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/m8-smoke-assertions.sh"
                ),
            ])
            .arg(dir.path())
            .env("HOME", dir.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(fs::read(dir.path().join("invocation.count")).unwrap(), b"x");
        assert_eq!(
            fs::read(dir.path().join("capture.out")).unwrap(),
            b"stdout\0exact\n"
        );
        assert_eq!(
            fs::read(dir.path().join("capture.err")).unwrap(),
            b"warning: stderr exact\n"
        );
    }
}
