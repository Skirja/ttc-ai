mod common;

use std::process::Command;

use common::ttc;

#[test]
fn unknown_binary_output_is_byte_exact_per_stream() {
    let script = "i=0; while [ \"$i\" -lt 20000 ]; do printf '\\377\\000out\\n'; printf '\\376\\000err\\r\\n' >&2; i=$((i+1)); done";
    let baseline = Command::new("/bin/sh")
        .args(["-c", script])
        .output()
        .unwrap();
    let wrapped = Command::new(ttc())
        .args(["/bin/sh", "-c", script])
        .output()
        .unwrap();

    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(wrapped.stdout, baseline.stdout);
    assert_eq!(wrapped.stderr, baseline.stderr);
    assert!(wrapped.stdout.len() > 16_000);
    assert!(wrapped.stderr.len() > 16_000);
}

#[test]
fn known_raw_command_preserves_exact_bytes() {
    let baseline = Command::new("/bin/printf")
        .args(["%s\\n", "α\nβ"])
        .output()
        .unwrap();
    let wrapped = Command::new(ttc())
        .args(["/bin/printf", "%s\\n", "α\nβ"])
        .output()
        .unwrap();
    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(wrapped.stdout, baseline.stdout);
    assert_eq!(wrapped.stderr, baseline.stderr);
}
