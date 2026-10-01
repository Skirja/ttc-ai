mod common;

use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use common::{TestDir, ttc_command};
use nix::sys::signal::{Signal, kill, killpg};
use nix::unistd::Pid;

#[test]
fn child_signal_status_matches_direct_baseline() {
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let script = format!("kill -{} $$", signal as i32);
        let baseline = Command::new("/bin/sh")
            .args(["-c", &script])
            .status()
            .unwrap();
        let wrapped = ttc_command()
            .args(["/bin/sh", "-c", &script])
            .status()
            .unwrap();
        assert_eq!(baseline.signal(), Some(signal as i32));
        assert_eq!(wrapped.signal(), baseline.signal());
    }
}

fn wait_for_exit(child: &mut Child, group: Pid) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = killpg(group, Signal::SIGKILL);
            let _ = child.kill();
            panic!("wrapper did not terminate after signal");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_pid(path: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // Creation and writing are separate operations. An existing empty
        // pidfile is not a readiness notification from the child.
        if let Ok(text) = fs::read_to_string(path)
            && let Ok(pid) = text.parse::<i32>()
            && pid > 0
        {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "child did not publish a valid PID"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn pid_readiness_waits_for_contents_after_file_creation() {
    let dir = TestDir::new();
    let path = dir.path().join("pid");
    fs::write(&path, b"").unwrap();
    let writer_path = path.clone();
    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(40));
        fs::write(writer_path, b"4242").unwrap();
    });
    assert_eq!(wait_for_pid(&path), 4242);
    writer.join().unwrap();
}

#[test]
fn signals_to_wrapper_reach_the_child_process_group() {
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let dir = TestDir::new();
        let pidfile = dir.path().join("child-pid");
        let script = "printf '%s' \"$$\" > \"$PIDFILE\"; exec sleep 30";
        let mut child = ttc_command()
            .args(["/bin/sh", "-c", script])
            .env("PIDFILE", &pidfile)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let child_pid = wait_for_pid(&pidfile);
        kill(Pid::from_raw(child.id() as i32), signal).unwrap();
        let status = wait_for_exit(&mut child, Pid::from_raw(child_pid));
        assert_eq!(status.signal(), Some(signal as i32));
    }
}
