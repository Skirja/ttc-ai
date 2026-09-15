//! TTC executable entrypoint.

mod cli;
mod core;
mod harness;

use std::process::ExitCode;

fn main() -> ExitCode {
    cli::run(std::env::args_os())
}
