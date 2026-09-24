mod common;

use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use common::{TestDir, ttc};
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
        let wrapped = Command::new(ttc())
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

#[test]
fn signals_to_wrapper_reach_the_child_process_group() {
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let dir = TestDir::new();
        let pidfile = dir.path().join("child-pid");
        let script = "printf '%s' \"$$\" > \"$PIDFILE\"; exec sleep 30";
        let mut child = Command::new(ttc())
            .args(["/bin/sh", "-c", script])
            .env("PIDFILE", &pidfile)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while !pidfile.exists() {
            assert!(Instant::now() < deadline, "child did not start");
            thread::sleep(Duration::from_millis(10));
        }
        let child_pid: i32 = fs::read_to_string(&pidfile).unwrap().parse().unwrap();
        kill(Pid::from_raw(child.id() as i32), signal).unwrap();
        let status = wait_for_exit(&mut child, Pid::from_raw(child_pid));
        assert_eq!(status.signal(), Some(signal as i32));
    }
}
