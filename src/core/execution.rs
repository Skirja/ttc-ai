//! Execute the original command once and stream its unmodified output.

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::thread;

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use signal_hook::low_level;

const COPY_BUFFER_SIZE: usize = 32 * 1024;

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

    if has_terminal()
        || may_start_background_shell_job(arguments)
        || is_known_raw_command(arguments)
    {
        let error = command.exec();
        return report_start_error(error);
    }

    stream_child(&mut command)
}

fn may_start_background_shell_job(arguments: &[OsString]) -> bool {
    // A background job can retain stdout/stderr after its shell exits. If TTC
    // owned those pipes, it could wait for that job and change shell timing.
    let shell_body = match arguments {
        [body] => Some(body),
        [program, option, body]
            if matches!(program.to_str(), Some("/bin/sh" | "sh")) && option == "-c" =>
        {
            Some(body)
        }
        _ => None,
    };
    shell_body
        .and_then(|body| body.to_str())
        .is_some_and(|body| body.contains('&'))
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

fn stream_child(command: &mut Command) -> ExitCode {
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
    let stdout_thread = thread::spawn(move || copy_raw(stdout, io::stdout()));
    let stderr_thread = thread::spawn(move || copy_raw(stderr, io::stderr()));

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

    match status {
        Ok(status) => exit_like_child(status),
        Err(error) => {
            eprintln!("ttc: waiting for command failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn copy_raw(mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    let mut buffer = [0u8; COPY_BUFFER_SIZE];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        output.write_all(&buffer[..read])?;
        output.flush()?;
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
