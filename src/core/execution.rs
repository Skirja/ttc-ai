//! Execute the original command once and stream its unmodified output.

use std::ffi::{OsStr, OsString};
use std::io::{self, IsTerminal};
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
        || classification::has_background_shell_job(arguments)
        || is_known_raw_command(arguments)
    {
        let error = command.exec();
        return report_start_error(error);
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let discovery = if classification::needs_manifest_discovery(arguments) {
        classification::ManifestHints::discover(&cwd)
    } else {
        Ok(None)
    };
    let mut plan = classification::classify(
        arguments,
        &cwd,
        discovery.as_ref().ok().and_then(Option::as_ref),
    );
    if discovery.is_err() {
        plan.raw = true;
    }
    stream_child(&mut command, config, DispatchFilter::new(plan))
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
            | "mysql"
            | "mariadb"
            | "psql"
            | "sqlite3"
            | "mongosh"
            | "redis-cli"
            | "sqlcmd"
            | "pgcli"
    ) || matches!(
        (name, next),
        ("npm" | "pnpm" | "yarn" | "bun", "run")
            if words.get(2).is_some_and(|word| *word == "dev")
    ) || matches!((name, next), ("npm" | "pnpm" | "yarn" | "bun", "dev"))
        || matches!((name, next), ("vite" | "next" | "nuxt", "dev"))
        || matches!((name, next), ("cargo", "watch"))
        || (name == "docker" && docker_compose_raw(&words[1..]))
        || (name == "kubectl" && kubectl_raw_interactive(&words[1..]))
        || (name == "git" && git_raw_subcommand(&words[1..]))
        || (name == "tail" && words.contains(&"-f"))
}

fn docker_compose_raw(arguments: &[&str]) -> bool {
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        if *argument == "compose" {
            index += 1;
            break;
        }
        if matches!(
            *argument,
            "--config"
                | "--context"
                | "--host"
                | "--log-level"
                | "--tlscacert"
                | "--tlscert"
                | "--tlskey"
        ) {
            index += 2;
        } else if [
            "--config=",
            "--context=",
            "--host=",
            "--log-level=",
            "--tlscacert=",
            "--tlscert=",
            "--tlskey=",
        ]
        .iter()
        .any(|prefix| argument.starts_with(prefix))
            || matches!(*argument, "--tls" | "--tlsverify" | "-D")
        {
            index += 1;
        } else {
            return false;
        }
    }
    if index == 0 || arguments.get(index - 1) != Some(&"compose") {
        return false;
    }
    while let Some(argument) = arguments.get(index) {
        if *argument == "up" {
            return true;
        }
        if *argument == "run" {
            return arguments[index + 1..].iter().any(|option| {
                matches!(
                    *option,
                    "-i" | "-t"
                        | "-it"
                        | "-ti"
                        | "--interactive"
                        | "--tty"
                        | "--interactive=true"
                        | "--tty=true"
                )
            });
        }
        if matches!(
            *argument,
            "-f" | "--file"
                | "--env-file"
                | "--project-directory"
                | "-p"
                | "--project-name"
                | "--profile"
                | "--progress"
                | "--ansi"
                | "--parallel"
        ) {
            index += 2;
        } else if argument.starts_with("--file=")
            || argument.starts_with("--env-file=")
            || argument.starts_with("--project-directory=")
            || argument.starts_with("--project-name=")
            || argument.starts_with("--profile=")
            || argument.starts_with("--progress=")
            || argument.starts_with("--ansi=")
            || argument.starts_with("--parallel=")
            || *argument == "--compatibility"
        {
            index += 1;
        } else {
            return false;
        }
    }
    false
}

fn kubectl_raw_interactive(arguments: &[&str]) -> bool {
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        if matches!(*argument, "logs" | "exec") {
            let subcommand = *argument;
            let options = &arguments[index + 1..];
            return if subcommand == "logs" {
                options
                    .iter()
                    .any(|option| matches!(*option, "-f" | "--follow" | "--follow=true"))
            } else {
                options.iter().any(|option| {
                    matches!(
                        *option,
                        "-i" | "-t"
                            | "-it"
                            | "-ti"
                            | "--stdin"
                            | "--tty"
                            | "--stdin=true"
                            | "--tty=true"
                    )
                })
            };
        }
        if matches!(
            *argument,
            "--as"
                | "--as-group"
                | "--cache-dir"
                | "--certificate-authority"
                | "--cluster"
                | "--client-certificate"
                | "--client-key"
                | "--context"
                | "--kubeconfig"
                | "--namespace"
                | "-n"
                | "--profile"
                | "--profile-output"
                | "--request-timeout"
                | "--server"
                | "--token"
                | "--tls-server-name"
                | "--user"
        ) {
            index += 2;
        } else if [
            "--as=",
            "--as-group=",
            "--cache-dir=",
            "--client-certificate=",
            "--client-key=",
            "--certificate-authority=",
            "--cluster=",
            "--context=",
            "--kubeconfig=",
            "--namespace=",
            "--profile=",
            "--profile-output=",
            "--request-timeout=",
            "--server=",
            "--token=",
            "--tls-server-name=",
            "--user=",
        ]
        .iter()
        .any(|prefix| argument.starts_with(prefix))
            || matches!(
                *argument,
                "--insecure-skip-tls-verify" | "--match-server-version" | "--warnings-as-errors"
            )
        {
            index += 1;
        } else {
            return false;
        }
    }
    false
}

fn git_raw_subcommand(arguments: &[&str]) -> bool {
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        if matches!(*argument, "diff" | "show") {
            return true;
        }
        if matches!(
            *argument,
            "-C" | "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--exec-path"
        ) {
            index += 2;
        } else if argument.starts_with("-C")
            || argument.starts_with("-c")
            || ["--git-dir=", "--work-tree=", "--namespace=", "--exec-path="]
                .iter()
                .any(|prefix| argument.starts_with(prefix))
            || matches!(
                *argument,
                "--no-pager"
                    | "--paginate"
                    | "--no-replace-objects"
                    | "--bare"
                    | "--literal-pathspecs"
                    | "--no-optional-locks"
            )
        {
            index += 1;
        } else {
            return false;
        }
    }
    false
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
    // Once the child has been reaped, forwarding is no longer needed. Some
    // restricted sandboxes block the wakeup used by signal-hook's iterator,
    // so joining that idle worker here can keep an otherwise-finished command
    // alive forever. Detach it; process exit reclaims the worker.
    drop(signal_thread);
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
