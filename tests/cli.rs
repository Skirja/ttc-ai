use std::process::{Command, Output};

const EXPECTED_HELP: &str = "\
TTC Automatic Bash Output Filter

Usage: ttc [OPTIONS]
       ttc <program> [args...]
       ttc '<complete shell command>'

Options:
  -h, --help     Print help
  -V, --version  Print version
";

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ttc"))
        .args(arguments)
        .output()
        .expect("ttc test binary should run")
}

#[test]
fn version_comes_from_the_package_version() {
    for argument in ["--version", "-V"] {
        let output = run(&[argument]);

        assert!(output.status.success());
        assert_eq!(output.stdout, b"ttc 0.1.0\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn help_only_lists_the_implemented_public_interface() {
    for arguments in [&["--help"][..], &["-h"][..], &[][..]] {
        let output = run(arguments);

        assert!(output.status.success());
        assert_eq!(output.stdout, EXPECTED_HELP.as_bytes());
        assert!(output.stderr.is_empty());
    }

    for unavailable_command in ["raw", "install", "uninstall", "hook", "explain", "doctor"] {
        assert!(!EXPECTED_HELP.contains(unavailable_command));
    }
}

#[test]
fn help_does_not_advertise_future_commands() {
    assert!(!EXPECTED_HELP.contains("raw"));
    assert!(!EXPECTED_HELP.contains("install"));
    assert!(!EXPECTED_HELP.contains("uninstall"));
    assert!(!EXPECTED_HELP.contains("hook"));
}
