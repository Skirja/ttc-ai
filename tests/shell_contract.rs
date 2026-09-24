mod common;

use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{TestDir, ttc_command};
use nix::pty::openpty;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

#[test]
fn one_argument_preserves_posix_shell_semantics() {
    let dir = TestDir::new();
    let baseline_file = dir.path().join("baseline-output");
    let wrapped_file = dir.path().join("wrapped-output");
    let baseline_count = dir.path().join("baseline-count");
    let wrapped_count = dir.path().join("wrapped-count");
    let script = r#"printf x >> "$COUNT_FILE"
FOO='sp ace' /bin/sh -c 'printf "%s\n" "$FOO"'
printf "single quote: ' and double \"quote\" λ\n"
printf 'first\nsecond\n' | sed -n '2p'
printf '%s\n' "$(printf substitution)"
printf 'redirected\n' > "$OUT_FILE" && cat "$OUT_FILE""#;

    let baseline = Command::new("/bin/sh")
        .args(["-c", script])
        .env("OUT_FILE", &baseline_file)
        .env("COUNT_FILE", &baseline_count)
        .output()
        .unwrap();
    let wrapped = ttc_command()
        .arg(script)
        .env("OUT_FILE", &wrapped_file)
        .env("COUNT_FILE", &wrapped_count)
        .output()
        .unwrap();

    assert!(baseline.status.success(), "baseline shell command failed");
    assert_eq!(wrapped.status, baseline.status);
    assert_eq!(wrapped.stdout, baseline.stdout);
    assert_eq!(wrapped.stderr, baseline.stderr);
    assert_eq!(
        fs::read(wrapped_file).unwrap(),
        fs::read(baseline_file).unwrap()
    );
    assert_eq!(fs::read(baseline_count).unwrap(), b"x");
    assert_eq!(fs::read(wrapped_count).unwrap(), b"x");
}

#[test]
fn raw_command_streams_before_stdin_closes() {
    let mut child = ttc_command()
        .args(["/bin/cat", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut bytes = [0u8; 6];
        stdout.read_exact(&mut bytes).unwrap();
        sender.send(bytes).unwrap();
    });
    child.stdin.as_mut().unwrap().write_all(b"ready\n").unwrap();
    let received = receiver.recv_timeout(Duration::from_secs(3));
    if received.is_err() {
        let _ = child.kill();
    }
    drop(child.stdin.take());
    assert_eq!(received.unwrap(), *b"ready\n");
    assert!(child.wait().unwrap().success());
    reader.join().unwrap();
}

#[test]
fn known_watch_command_execs_in_place() {
    let dir = TestDir::new();
    let fake_npm = dir.path().join("npm");
    fs::write(&fake_npm, b"#!/bin/sh\nprintf '%s' \"$$\"\n").unwrap();
    fs::set_permissions(&fake_npm, fs::Permissions::from_mode(0o755)).unwrap();

    let mut child = ttc_command()
        .arg(&fake_npm)
        .args(["run", "dev"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let wrapper_pid = child.id();
    let mut output = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(output.parse::<u32>().unwrap(), wrapper_pid);
}

#[test]
fn terminal_is_inherited_by_the_command() {
    let pty = openpty(None, None).unwrap();
    let mut master = File::from(pty.master);
    let slave = File::from(pty.slave);
    let mut command = ttc_command();
    command
        .args([
            "/bin/sh",
            "-c",
            "test -t 0 && test -t 1 && test -t 2 && printf tty",
        ])
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave.try_clone().unwrap()));
    let mut child = command.spawn().unwrap();
    drop(slave);
    assert!(child.wait().unwrap().success());
    let mut bytes = [0u8; 3];
    master.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"tty");
}

#[test]
fn background_shell_job_does_not_delay_wrapper_exit() {
    const SCRIPT: &str = "sleep 30 & printf '%s' \"$!\" > \"$BG_PID\"";
    for arguments in [
        &[SCRIPT][..],
        &["bash", "-c", SCRIPT][..],
        &["/usr/bin/env", "bash", "-c", SCRIPT][..],
    ] {
        let dir = TestDir::new();
        let pidfile = dir.path().join("background-pid");
        let mut child = ttc_command()
            .args(arguments)
            .env("BG_PID", &pidfile)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                stop_background_job(&pidfile);
                let _ = child.kill();
                let _ = child.wait();
                panic!("TTC waited for a background job: {arguments:?}");
            }
            thread::sleep(Duration::from_millis(10));
        };
        stop_background_job(&pidfile);
        assert!(status.success(), "shell command failed: {arguments:?}");
    }
}

fn stop_background_job(pidfile: &Path) {
    if let Ok(pid) = fs::read_to_string(pidfile)
        && let Ok(pid) = pid.parse::<i32>()
    {
        let _ = kill(Pid::from_raw(pid), Signal::SIGTERM);
    }
}
