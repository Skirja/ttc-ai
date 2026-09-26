//! Execute the original command once and stream its unmodified output.

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use signal_hook::low_level;

use super::classification;
use super::config::Config;
use super::filters::DispatchFilter;
use super::raw_store::{self, StorePaths, Stream};
use super::streaming;

pub(crate) fn run(arguments: &[OsString]) -> ExitCode {
    let mut command = if arguments.len() == 1 {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(&arguments[0]);
        command
    } else {
        let mut command = Command::new(&arguments[0]);
        command.args(&arguments[1..]);
        command
    };

    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => {
            let error = command.exec();
            return report_start_error(error);
        }
    };
    raw_store::cleanup(config);

    if has_terminal()
        || may_start_background_shell_job(arguments)
        || is_known_raw_command(arguments)
    {
        let error = command.exec();
        return report_start_error(error);
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let hints = classification::ManifestHints::load(&cwd);
    let plan = classification::classify(arguments, &cwd, hints.as_ref());
    stream_child(&mut command, config, DispatchFilter::new(plan))
}

fn may_start_background_shell_job(arguments: &[OsString]) -> bool {
    // The shell expression may be nested under bash, env, or another wrapper.
    // A background job can retain output descriptors after its parent exits,
    // so conservatively preserve the original descriptors when any argument
    // contains an ampersand. No filtering is active in M2.
    arguments
        .iter()
        .any(|argument| argument.as_bytes().contains(&b'&'))
}

fn has_terminal() -> bool {
    io::stdin().is_terminal() || io::stdout().is_terminal() || io::stderr().is_terminal()
}

fn is_known_raw_command(arguments: &[OsString]) -> bool {
    let words: Vec<&str> = if arguments.len() == 1 {
        // Only simple, literal shell commands are recognized here. An
        // unrecognized expression still streams raw through both pipes.
        let Some(shell_command) = arguments[0].to_str() else {
            return false;
        };
        if shell_command.contains(['\'', '"', '$', ';', '|', '&', '>', '<', '\n']) {
            return false;
        }
        shell_command.split_ascii_whitespace().collect()
    } else {
        let Some(words) = arguments.iter().map(|word| word.to_str()).collect() else {
            return false;
        };
        words
    };

    let Some(program) = words.first() else {
        return false;
    };
    let name = Path::new(program)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(program);
    let next = words.get(1).copied().unwrap_or_default();
    let third = words.get(2).copied().unwrap_or_default();

    matches!(
        name,
        "cat"
            | "bat"
            | "sed"
            | "awk"
            | "head"
            | "tail"
            | "less"
            | "more"
            | "file"
            | "strings"
            | "base64"
            | "xxd"
            | "hexdump"
            | "env"
            | "printenv"
            | "echo"
            | "printf"
            | "curl"
            | "wget"
            | "ssh"
            | "scp"
            | "watchexec"
            | "nodemon"
    ) || matches!(
        (name, next, third),
        ("npm" | "pnpm" | "yarn" | "bun", "run", "dev")
            | ("npm" | "pnpm" | "yarn" | "bun", "dev", _)
            | ("vite" | "next" | "nuxt", "dev", _)
            | ("cargo", "watch", _)
            | ("docker", "compose", "up")
            | ("kubectl", "logs", "-f")
            | ("git", "diff" | "show", _)
    ) || (name == "tail" && words.contains(&"-f"))
}

fn stream_child(command: &mut Command, config: Config, mut filter: DispatchFilter) -> ExitCode {
    // Installing signal handlers before spawn closes the gap in which a
    // signal could otherwise terminate the wrapper but leave the child alive.
    let mut signals = match Signals::new([SIGINT, SIGTERM]) {
        Ok(signals) => signals,
        Err(error) => return report_start_error(error),
    };
    let signal_handle = signals.handle();

    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return report_start_error(error),
    };

    let group = Pid::from_raw(child.id() as i32);
    let signal_thread = thread::spawn(move || {
        for number in signals.forever() {
            if let Ok(signal) = Signal::try_from(number) {
                let _ = killpg(group, signal);
            }
        }
    });

    let stdout = child.stdout.take().expect("piped stdout is available");
    let stderr = child.stderr.take().expect("piped stderr is available");
    let (sender, receiver) = mpsc::sync_channel(8);
    let stderr_sender = sender.clone();
    let stdout_thread =
        thread::spawn(move || streaming::read_stream(stdout, Stream::Stdout, sender));
    let stderr_thread =
        thread::spawn(move || streaming::read_stream(stderr, Stream::Stderr, stderr_sender));
    let report = streaming::process(receiver, config, StorePaths::from_env().ok(), &mut filter);
    let status = child.wait();
    signal_handle.close();
    let _ = signal_thread.join();
    let stdout_result = stdout_thread.join();
    let stderr_result = stderr_thread.join();

    for result in [stdout_result, stderr_result] {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) if error.kind() == io::ErrorKind::BrokenPipe => {}
            Ok(Err(error)) => eprintln!("ttc: output forwarding failed: {error}"),
            Err(_) => eprintln!("ttc: output forwarding thread panicked"),
        }
    }
    if let Some(error) = &report.forwarding_error {
        eprintln!("ttc: output forwarding failed: {error}");
    }
    if let Err(error) = streaming::write_metadata(&report, &mut io::stderr()) {
        eprintln!("ttc: writing summary failed: {error}");
    }

    match status {
        Ok(status) => exit_like_child(status),
        Err(error) => {
            eprintln!("ttc: waiting for command failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn exit_like_child(status: ExitStatus) -> ExitCode {
    if let Some(code) = status.code() {
        return ExitCode::from(u8::try_from(code).unwrap_or(1));
    }
    if let Some(signal) = status.signal() {
        // This does not return for terminating signals. It preserves the
        // signal in the status observed by TTC's caller.
        let _ = low_level::emulate_default_handler(signal);
        return ExitCode::from(u8::try_from(128 + signal).unwrap_or(1));
    }
    ExitCode::FAILURE
}

fn report_start_error(error: io::Error) -> ExitCode {
    eprintln!("ttc: cannot start command: {error}");
    if error.kind() == io::ErrorKind::NotFound {
        ExitCode::from(127)
    } else {
        ExitCode::from(126)
    }
}
