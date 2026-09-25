//! Concurrent bounded input and one output decision point.

use std::io::{self, Read, Write};
use std::sync::mpsc::{Receiver, SyncSender};

use super::config::Config;
use super::raw_store::{Capture, StorePaths, Stream};

pub(crate) const COPY_BUFFER_SIZE: usize = 32 * 1024;
const MAX_PENDING: usize = 1024 * 1024;

pub(crate) type Event = (Stream, Vec<u8>);

#[derive(Clone, Copy)]
pub(crate) enum CompactKind {
    Passing,
    Progress,
}

pub(crate) trait Filter {
    fn can_compact(&self) -> bool;
    fn decide(&mut self, stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()>;
}

#[cfg(test)]
#[allow(dead_code)] // The integration tests exercise the raw filter boundary.
pub(crate) struct Retain;

#[cfg(test)]
impl Filter for Retain {
    fn can_compact(&self) -> bool {
        false
    }
    fn decide(&mut self, _stream: Stream, _line: &[u8]) -> Result<Option<CompactKind>, ()> {
        Ok(None)
    }
}

#[derive(Default, Debug)]
pub(crate) struct Report {
    pub passing: u64,
    pub progress: u64,
    pub raw_id: Option<String>,
    pub forwarding_error: Option<io::Error>,
}

pub(crate) fn write_metadata(report: &Report, output: &mut impl Write) -> io::Result<()> {
    if let Some(id) = &report.raw_id {
        writeln!(
            output,
            "TTC: {} passing records and {} progress records compacted",
            report.passing, report.progress
        )?;
        writeln!(output, "raw: ttc raw {id}")?;
    }
    Ok(())
}

pub(crate) fn read_stream(
    mut input: impl Read,
    stream: Stream,
    sender: SyncSender<Event>,
) -> io::Result<()> {
    let mut buffer = [0u8; COPY_BUFFER_SIZE];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        if sender.send((stream, buffer[..count].to_vec())).is_err() {
            return Ok(());
        }
    }
}

pub(crate) fn process(
    receiver: Receiver<Event>,
    config: Config,
    paths: Option<StorePaths>,
    filter: &mut impl Filter,
) -> Report {
    process_to(
        receiver,
        config,
        paths,
        filter,
        &mut io::stdout(),
        &mut io::stderr(),
    )
}

pub(crate) fn process_to(
    receiver: Receiver<Event>,
    config: Config,
    paths: Option<StorePaths>,
    filter: &mut impl Filter,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> Report {
    let mut report = Report::default();
    let mut capture = if filter.can_compact() {
        paths.map(|paths| Capture::new(config, paths))
    } else {
        None
    };
    let mut pending = [Vec::new(), Vec::new()];
    let mut raw = capture.is_none();
    for (stream, bytes) in receiver {
        if raw {
            emit(stream, &bytes, &mut report, stdout, stderr);
            continue;
        }
        if capture.as_mut().unwrap().record(stream, &bytes).is_err() {
            raw = true;
            flush_pending(&mut pending, &mut report, stdout, stderr);
            emit(stream, &bytes, &mut report, stdout, stderr);
            continue;
        }
        let index = if stream == Stream::Stdout { 0 } else { 1 };
        let mut position = 0;
        while position < bytes.len() {
            let rest = &bytes[position..];
            let length = rest
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(rest.len(), |offset| offset + 1);
            let part = &rest[..length];
            if pending[index].len() + part.len() > MAX_PENDING {
                raw = true;
                flush_pending(&mut pending, &mut report, stdout, stderr);
                emit(stream, &bytes[position..], &mut report, stdout, stderr);
                break;
            }
            pending[index].extend_from_slice(part);
            position += length;
            if part.last() != Some(&b'\n') {
                continue;
            }
            if std::str::from_utf8(&pending[index]).is_err() {
                raw = true;
                flush_pending(&mut pending, &mut report, stdout, stderr);
                emit(stream, &bytes[position..], &mut report, stdout, stderr);
                break;
            }
            match filter.decide(stream, &pending[index]) {
                Ok(None) => emit(stream, &pending[index], &mut report, stdout, stderr),
                Ok(Some(kind)) => {
                    if capture.as_mut().unwrap().enable(config).is_ok() {
                        match kind {
                            CompactKind::Passing => report.passing += 1,
                            CompactKind::Progress => report.progress += 1,
                        }
                    } else {
                        raw = true;
                        emit(stream, &pending[index], &mut report, stdout, stderr);
                    }
                }
                Err(()) => {
                    raw = true;
                    emit(stream, &pending[index], &mut report, stdout, stderr);
                }
            }
            pending[index].clear();
            if raw {
                flush_pending(&mut pending, &mut report, stdout, stderr);
                emit(stream, &bytes[position..], &mut report, stdout, stderr);
                break;
            }
        }
    }
    flush_pending(&mut pending, &mut report, stdout, stderr);
    if report.passing + report.progress > 0 {
        match capture.as_mut().unwrap().finish() {
            Ok(Some(id)) => report.raw_id = Some(id.to_owned()),
            Ok(None) => {}
            Err(error) => report.forwarding_error = Some(error),
        }
    }
    report
}

fn flush_pending(
    pending: &mut [Vec<u8>; 2],
    report: &mut Report,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) {
    for (index, bytes) in pending.iter_mut().enumerate() {
        if !bytes.is_empty() {
            emit(
                if index == 0 {
                    Stream::Stdout
                } else {
                    Stream::Stderr
                },
                bytes,
                report,
                stdout,
                stderr,
            );
            bytes.clear();
        }
    }
}

fn emit(
    stream: Stream,
    bytes: &[u8],
    report: &mut Report,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) {
    if report.forwarding_error.is_some() {
        return;
    }
    let result = match stream {
        Stream::Stdout => stdout.write_all(bytes).and_then(|()| stdout.flush()),
        Stream::Stderr => stderr.write_all(bytes).and_then(|()| stderr.flush()),
    };
    if let Err(error) = result
        && error.kind() != io::ErrorKind::BrokenPipe
    {
        report.forwarding_error = Some(error);
    }
}
