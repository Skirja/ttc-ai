//! Public command-line contract for the functionality implemented so far.

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

const HELP: &str = "\
TTC Automatic Bash Output Filter

Usage: ttc [OPTIONS]

Options:
  -h, --help     Print help
  -V, --version  Print version
";

const UNSUPPORTED_ARGUMENTS: &str =
    "error: unsupported arguments\n\nFor more information, try '--help'.";

/// Runs the top-level CLI parser.
///
/// Execution commands are intentionally rejected until their contracts are
/// implemented by the corresponding milestone.
pub(crate) fn run(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let mut arguments = arguments.into_iter();
    let _executable = arguments.next();

    let first = arguments.next();
    let has_more = arguments.next().is_some();

    match (first, has_more) {
        (None, false) => print_help(),
        (Some(argument), false)
            if argument == OsStr::new("--help") || argument == OsStr::new("-h") =>
        {
            print_help()
        }
        (Some(argument), false)
            if argument == OsStr::new("--version") || argument == OsStr::new("-V") =>
        {
            println!("ttc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{UNSUPPORTED_ARGUMENTS}");
            ExitCode::from(2)
        }
    }
}

fn print_help() -> ExitCode {
    print!("{HELP}");
    ExitCode::SUCCESS
}
