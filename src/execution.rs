//! Execute once, capture incrementally, then render without changing child outcomes.
use crate::{
    command::*,
    config::Config,
    filtering::Filter,
    recovery::{Record, Store, now},
    resolver::classify_cached,
};
use anyhow::{Context, Result};
use std::{
    io::{IsTerminal, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Instant,
};
fn emit(event: OutputEvent) -> u64 {
    let result = match event.channel {
        Channel::Stdout => std::io::stdout().write_all(&event.bytes),
        Channel::Stderr => std::io::stderr().write_all(&event.bytes),
    };
    // Keep draining and reap the child even if the downstream reader disconnects.
    if result.is_ok() {
        event.bytes.len() as u64
    } else {
        0
    }
}
fn passthrough(cmd: &mut Command) -> Result<ProcessOutcome> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(cmd.exec().into())
    }
    #[cfg(not(unix))]
    {
        Ok(ProcessOutcome::from_status(cmd.status()?))
    }
}
fn command(inv: &Invocation) -> Result<Command> {
    match inv {
        Invocation::Argv(a) => {
            let mut c = Command::new(a.first().context("missing program")?);
            c.args(&a[1..]);
            Ok(c)
        }
        Invocation::Shell {
            executable,
            dialect,
            command,
        } => {
            let mut c = Command::new(executable);
            match dialect {
                Shell::Posix => {
                    c.arg("-c");
                }
                Shell::PowerShell => {
                    c.args(["-NoProfile", "-Command"]);
                }
                Shell::Cmd => {
                    c.args(["/D", "/S", "/C"]);
                }
            }
            c.arg(command);
            Ok(c)
        }
    }
}
pub fn run(inv: Invocation, cwd: &Path, cfg: &Config, raw: bool) -> Result<ProcessOutcome> {
    let dialect = match &inv {
        Invocation::Shell { dialect, .. } => *dialect,
        _ => Shell::Posix,
    };
    let mut cmd = command(&inv)?;
    cmd.current_dir(cwd).stdin(Stdio::inherit());
    if raw || !cfg.enabled || !cfg.recovery.enabled || std::io::stdout().is_terminal() {
        return passthrough(&mut cmd);
    }
    let display = inv.display();
    let class = classify_cached(
        &display,
        cwd,
        dialect,
        &crate::config::data_dir().join("discovery"),
    );
    if !class.filterable {
        return passthrough(&mut cmd);
    }
    let Ok(store) = Store::open() else {
        return passthrough(&mut cmd);
    };
    let _ = store.clean(cfg, false);
    let Ok(mut capture) = store.capture(cfg) else {
        return passthrough(&mut cmd);
    };
    let started = Instant::now();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let mut evidence = ExecutionEvidence::top_level_started();
    #[cfg(unix)]
    let signal_thread = {
        use signal_hook::{
            consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
            iterator::Signals,
        };
        let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP, SIGQUIT])?;
        let handle = signals.handle();
        let pid = child.id() as i32;
        let t = thread::spawn(move || {
            for signal in signals.forever() {
                // The piped child owns a fresh process group; forward to its descendants.
                unsafe {
                    libc::kill(-pid, signal);
                }
            }
        });
        (handle, t)
    };
    let (tx, rx) = mpsc::sync_channel::<Result<OutputEvent>>(8);
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;
    fn reader<R: Read + Send + 'static>(
        mut r: R,
        channel: Channel,
        tx: mpsc::SyncSender<Result<OutputEvent>>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut b = [0u8; 16384];
            loop {
                match r.read(&mut b) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx
                            .send(Ok(OutputEvent {
                                channel,
                                bytes: b[..n].to_vec(),
                            }))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                        break;
                    }
                }
            }
        })
    }
    let t1 = reader(stdout, Channel::Stdout, tx.clone());
    let t2 = reader(stderr, Channel::Stderr, tx.clone());
    drop(tx);
    let mut partial = false;
    let mut raw_bytes = 0u64;
    let mut output_bytes = 0u64;
    let mut filter = Filter::new(class.families.clone());
    let mut raw_mode = false;
    for item in rx {
        let event = match item {
            Ok(e) => e,
            Err(e) => {
                eprintln!("TTC output read error: {e}");
                partial = true;
                continue;
            }
        };
        evidence.observe(&event);
        raw_bytes += event.bytes.len() as u64;
        if !partial && let Err(error) = capture.write(&event) {
            partial = true;
            eprintln!("TTC capture limit/failure ({error}); uncaptured output is raw");
        }
        if (partial || capture.binary) && !raw_mode {
            for pending in filter.finish() {
                output_bytes += emit(pending);
            }
            raw_mode = true;
        }
        if raw_mode {
            output_bytes += emit(event);
        } else {
            // A flushed raw frame exists before the filter is allowed to suppress it.
            for event in filter.feed(event) {
                output_bytes += emit(event);
            }
        }
    }
    let _ = t1.join();
    let _ = t2.join();
    let outcome = ProcessOutcome::from_status(child.wait()?);
    evidence.finish(outcome.clone(), started.elapsed().as_millis() as u64);
    #[cfg(unix)]
    {
        signal_thread.0.close();
        let _ = signal_thread.1.join();
    }
    if !raw_mode {
        for event in filter.finish() {
            output_bytes += emit(event);
        }
    }
    let suppressed = filter.stats().suppressed_lines > 0
        || filter.stats().rewritten_lines > 0
        || filter.stats().duplicate_diagnostics > 0;
    if let Err(e) = capture.seal() {
        eprintln!("TTC capture finalization failed: {e}");
        partial = true;
    }
    let keep = suppressed || !outcome.success() || partial;
    let capture_id = capture.id.clone();
    let path = capture.path.clone();
    let hint = if keep {
        format!(
            "\nraw{}: ttc recall {capture_id}\n",
            if partial { " (partial)" } else { "" }
        )
    } else {
        String::new()
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    let record = Record {
        id: capture_id,
        created: now(),
        cwd: cwd.to_string_lossy().into(),
        command: display,
        outcome: outcome.clone(),
        raw_bytes,
        filtered_bytes: output_bytes + hint.len() as u64,
        duration_ms,
        families: class.families,
        partial,
        execution_evidence: Some(evidence),
    };
    if keep {
        match store.publish(capture, &record) {
            Ok(()) => {
                eprint!("{hint}");
            }
            Err(e) => {
                eprintln!(
                    "TTC could not publish recall: {e}; temporary capture: {}",
                    path.display()
                );
            }
        }
    } else {
        drop(capture);
        let _ = std::fs::remove_file(path);
    }
    if cfg.metrics {
        let _ = store.metric(&record);
    }
    let _ = store.clean(cfg, false);
    Ok(outcome)
}
/// Preserve Unix signal termination rather than guessing from numeric exit codes.
pub fn exit(outcome: ProcessOutcome) -> ! {
    #[cfg(unix)]
    if let Some(signal) = outcome.signal {
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
        std::process::exit(128 + signal);
    }
    std::process::exit(outcome.code.unwrap_or(1))
}
