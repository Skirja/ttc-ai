//! Public command-line contract for the functionality implemented so far.

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use crate::core::execution;

const HELP: &str = "\
TTC Automatic Bash Output Filter

Usage: ttc [OPTIONS]
       ttc <program> [args...]
       ttc '<complete shell command>'

Options:
  -h, --help     Print help
  -V, --version  Print version
";

/// Runs the top-level CLI parser.
pub(crate) fn run(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let mut arguments = arguments.into_iter();
    let _executable = arguments.next();
    let command: Vec<_> = arguments.collect();

    match command.as_slice() {
        [] => print_help(),
        [argument] if argument == OsStr::new("--help") || argument == OsStr::new("-h") => {
            print_help()
        }
        [argument] if argument == OsStr::new("--version") || argument == OsStr::new("-V") => {
            println!("ttc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => execution::run(&command),
    }
}

fn print_help() -> ExitCode {
    print!("{HELP}");
    ExitCode::SUCCESS
}
