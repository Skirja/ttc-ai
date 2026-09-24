//! Public command-line contract for the functionality implemented so far.

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use crate::core::execution;
use crate::core::raw_store::{self, Selection};

const HELP: &str = "\
TTC Automatic Bash Output Filter

Usage: ttc [OPTIONS]
       ttc <program> [args...]
       ttc '<complete shell command>'
       ttc raw <id> [--stdout | --stderr] [--tail N]

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
        [argument, ..] if argument == OsStr::new("raw") => run_raw(&command[1..]),
        _ => execution::run(&command),
    }
}

fn run_raw(arguments: &[OsString]) -> ExitCode {
    let Some(id) = arguments.first().and_then(|argument| argument.to_str()) else {
        eprintln!("ttc: usage: ttc raw <id> [--stdout | --stderr] [--tail N]");
        return ExitCode::from(2);
    };
    let mut selection = Selection::Both;
    let mut selected = false;
    let mut tail = None;
    let mut index = 1;
    while index < arguments.len() {
        match arguments[index].to_str() {
            Some("--stdout") if !selected => {
                selection = Selection::Stdout;
                selected = true;
            }
            Some("--stderr") if !selected => {
                selection = Selection::Stderr;
                selected = true;
            }
            Some("--tail") if tail.is_none() && index + 1 < arguments.len() => {
                index += 1;
                tail = arguments[index]
                    .to_str()
                    .and_then(|value| value.parse::<u64>().ok());
                if tail.is_none() {
                    return raw_usage();
                }
            }
            _ => return raw_usage(),
        }
        index += 1;
    }
    match raw_store::replay(id, selection, tail) {
        Ok(dropped) => {
            if dropped > 0 {
                eprintln!("ttc raw: {dropped} original bytes dropped from capture");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("ttc raw: {error}");
            ExitCode::FAILURE
        }
    }
}

fn raw_usage() -> ExitCode {
    eprintln!("ttc: usage: ttc raw <id> [--stdout | --stderr] [--tail N]");
    ExitCode::from(2)
}

fn print_help() -> ExitCode {
    print!("{HELP}");
    ExitCode::SUCCESS
}
