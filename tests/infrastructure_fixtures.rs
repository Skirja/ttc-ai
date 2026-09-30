#![allow(dead_code)]
#[path = "../src/core/mod.rs"]
mod core;
#[path = "common/m8_support.rs"]
mod m8_support;

use core::classification::Family;
use m8_support::run_filter;

#[test]
fn helm_chart_progress_compacts_while_lint_diagnostics_and_summary_stay() {
    let lines = (0..1001)
        .map(|_| ("stdout", "==> Linting charts/demo\n"))
        .chain([("stdout", "[INFO] Chart.yaml: icon is recommended\n")])
        .chain([("stdout", "1 chart(s) linted, 0 chart(s) failed\n")])
        .chain([("stderr", "Error: chart template is invalid\n")])
        .collect::<Vec<_>>();
    let result = run_filter(Family::HelmLint, &lines);
    assert_eq!(result.report.progress, 998);
    assert!(result.report.raw_id.is_some());
    assert!(
        result
            .stdout
            .ends_with(b"1 chart(s) linted, 0 chart(s) failed\n")
    );
    assert_eq!(result.stderr, b"Error: chart template is invalid\n");
    assert!(
        (result.stdout.len() + result.stderr.len() + result.metadata.len()) * 5
            < lines.iter().map(|(_, line)| line.len()).sum::<usize>()
    );
}

#[test]
fn terraform_validate_is_diagnostic_only_and_never_compacts() {
    let lines = [
        ("stdout", "Success! The configuration is valid.\n"),
        ("stderr", "Warning: Deprecated argument\n"),
        ("stderr", "Error: Invalid resource\n  on main.tf line 7\n"),
    ];
    let result = run_filter(Family::TerraformValidate, &lines);
    assert_eq!(result.stdout, lines[0].1.as_bytes());
    assert_eq!(result.stderr, [lines[1].1, lines[2].1].concat().as_bytes());
    assert!(result.report.raw_id.is_none());
    assert!(result.metadata.is_empty());
}

#[test]
fn buildkit_recognizer_does_not_compact_application_steps_or_security_output() {
    for line in [
        "#2 [build 1/2] RUN npm install\n",
        "#3 [build 2/2] COPY src/ /app\n",
        "#4 [internal] load build context\nWarning: untrusted archive\n",
        "time=\"2026-09-30T08:06:27Z\" level=error msg=\"Can't read security.capability attribute\"\n",
    ] {
        let result = run_filter(Family::ContainerBuild, &[("stdout", line)]);
        assert_eq!(result.stdout, line.as_bytes());
        assert!(result.report.raw_id.is_none());
        assert!(result.metadata.is_empty());
    }
}

#[test]
fn repeated_safe_buildkit_progress_reduces_while_steps_stay_visible() {
    let lines = (0..1001)
        .map(|_| {
            (
                "stdout",
                "#1 [internal] load build definition from Dockerfile\n",
            )
        })
        .chain([("stdout", "#2 [build 1/1] RUN make release\n")])
        .chain([("stdout", "#3 exporting to image\n")])
        .chain([("stderr", "warning: preserve builder warning\n")])
        .collect::<Vec<_>>();
    let result = run_filter(Family::ContainerBuild, &lines);
    assert_eq!(result.report.progress, 998);
    assert!(result.report.raw_id.is_some());
    assert!(result.stdout.ends_with(b"#3 exporting to image\n"));
    assert_eq!(result.stderr, b"warning: preserve builder warning\n");
    assert!(
        (result.stdout.len() + result.stderr.len() + result.metadata.len()) * 5
            < lines.iter().map(|(_, line)| line.len()).sum::<usize>()
    );
}
