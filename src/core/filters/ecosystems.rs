//! Conservative Rust, Python, and Go output recognizers.

use std::collections::HashSet;

use serde_json::Value;

use super::super::classification::{Family, Plan};
use super::super::raw_store::Stream;
use super::super::streaming::{CompactKind, Filter};
use super::{protected, strip_ansi};

const TEXT_PARSER_COUNT: usize = 7;
const GO_TEST_PARSER: usize = 6;

pub(crate) struct EcosystemFilter {
    plan: Plan,
    confidence: [[u8; TEXT_PARSER_COUNT]; 2],
    json_confidence: u8,
    diagnostic_block: [[bool; TEXT_PARSER_COUNT]; 2],
}

impl EcosystemFilter {
    pub(crate) fn new(plan: Plan) -> Self {
        Self {
            plan,
            confidence: [[0; TEXT_PARSER_COUNT]; 2],
            json_confidence: 0,
            diagnostic_block: [[false; TEXT_PARSER_COUNT]; 2],
        }
    }

    fn has_json_parser(&self) -> bool {
        !self.plan.raw && self.plan.families.contains(&Family::GoJson)
    }

    fn has_text_parser(&self) -> bool {
        !self.plan.raw
            && self
                .plan
                .families
                .iter()
                .any(|family| text_parser_index(*family).is_some())
    }
}

impl Filter for EcosystemFilter {
    fn can_compact(&self) -> bool {
        self.has_json_parser() || self.has_text_parser()
    }

    fn decide(&mut self, stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        if self.has_json_parser() {
            if stream == Stream::Stderr {
                return Ok(None);
            }
            return self.decide_go_json(line);
        }

        let stream_index = if stream == Stream::Stdout { 0 } else { 1 };
        let mut active = [false; TEXT_PARSER_COUNT];
        for family in &self.plan.families {
            if let Some(index) = text_parser_index(*family) {
                active[index] = true;
            }
        }
        let text = strip_ansi(line).ok_or(())?;
        let text = text.trim_end_matches(['\r', '\n']);
        let record = text.trim();
        if record.is_empty() {
            // Failure reports can separate diagnostics from captured output
            // with blank lines, so only reset confidence here.
            for (index, is_active) in active.iter().copied().enumerate() {
                if is_active {
                    self.confidence[stream_index][index] = 0;
                }
            }
            return Ok(None);
        }
        if protected(record) {
            for (index, is_active) in active.iter().copied().enumerate() {
                if is_active {
                    self.confidence[stream_index][index] = 0;
                    self.diagnostic_block[stream_index][index] = true;
                }
            }
            return Ok(None);
        }
        if active
            .iter()
            .copied()
            .enumerate()
            .any(|(index, is_active)| is_active && self.diagnostic_block[stream_index][index])
        {
            for (index, is_active) in active.iter().copied().enumerate() {
                if is_active {
                    self.confidence[stream_index][index] = 0;
                }
            }
            return Ok(None);
        }

        if self.plan.families.contains(&Family::GoTest) && go_test_lifecycle(record) {
            for (index, is_active) in active.iter().copied().enumerate() {
                if is_active && index != GO_TEST_PARSER {
                    self.confidence[stream_index][index] = 0;
                }
            }
            return Ok(None);
        }

        for family in &self.plan.families {
            let Some(parser_index) = text_parser_index(*family) else {
                continue;
            };
            let kind = match family {
                Family::RustTest if rust_test_pass(record) => Some(CompactKind::Passing),
                Family::RustNextest if nextest_pass(record) => Some(CompactKind::Passing),
                Family::RustBuild | Family::RustCheck | Family::RustClippy | Family::RustDoc
                    if cargo_progress(record, *family) =>
                {
                    Some(CompactKind::Progress)
                }
                Family::PyTest if pytest_pass(record) => Some(CompactKind::Passing),
                Family::PyUnittest if unittest_pass(record) => Some(CompactKind::Passing),
                Family::PyInstall if python_install_progress(record) => Some(CompactKind::Progress),
                Family::GoTest if go_test_pass(record) => Some(CompactKind::Passing),
                _ => None,
            };
            if let Some(kind) = kind {
                for (index, is_active) in active.iter().copied().enumerate() {
                    if is_active && index != parser_index {
                        self.confidence[stream_index][index] = 0;
                    }
                }
                self.confidence[stream_index][parser_index] =
                    self.confidence[stream_index][parser_index].saturating_add(1);
                return Ok((self.confidence[stream_index][parser_index] > 3).then_some(kind));
            }
        }
        for (index, is_active) in active.iter().copied().enumerate() {
            if is_active {
                self.confidence[stream_index][index] = 0;
            }
        }
        Ok(None)
    }
}

fn text_parser_index(family: Family) -> Option<usize> {
    match family {
        Family::RustTest => Some(0),
        Family::RustNextest => Some(1),
        Family::RustBuild | Family::RustCheck | Family::RustClippy | Family::RustDoc => Some(2),
        Family::PyTest => Some(3),
        Family::PyUnittest => Some(4),
        Family::PyInstall => Some(5),
        Family::GoTest => Some(GO_TEST_PARSER),
        _ => None,
    }
}

fn rust_test_pass(line: &str) -> bool {
    let Some(name) = line
        .strip_prefix("test ")
        .and_then(|rest| rest.strip_suffix(" ... ok"))
    else {
        return false;
    };
    !name.is_empty() && name.len() <= 512 && !name.contains(" ... ")
}

fn nextest_pass(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("PASS ") else {
        return false;
    };
    !rest.is_empty()
        && rest.len() <= 512
        && !rest.contains(['\r', '\n'])
        && (rest.contains("::") || rest.starts_with('['))
        && !rest.to_ascii_lowercase().contains("warning")
}

fn cargo_progress(line: &str, family: Family) -> bool {
    let prefixes: &[&str] = if family == Family::RustDoc {
        &["Compiling ", "Checking ", "Documenting "]
    } else {
        &["Compiling ", "Checking "]
    };
    prefixes.iter().any(|prefix| {
        line.strip_prefix(prefix).is_some_and(|rest| {
            rest.len() <= 512
                && rest.contains(" v")
                && rest.contains(" (")
                && rest.ends_with(')')
                && !rest.contains(['\r', '\n'])
        })
    })
}

fn pytest_pass(line: &str) -> bool {
    let status = line
        .rsplit_once(" [")
        .filter(|(_, suffix)| {
            suffix
                .strip_suffix(']')
                .map(str::trim)
                .and_then(|digits| digits.strip_suffix('%'))
                .and_then(|digits| digits.parse::<u8>().ok())
                .is_some()
        })
        .map_or(line, |(record, _)| record);
    status.trim_end().ends_with(" PASSED")
        && status.contains("::")
        && status.len() <= 2048
        && !status.contains("ERROR")
}

fn unittest_pass(line: &str) -> bool {
    let Some((name, status)) = line.rsplit_once(" ... ") else {
        return false;
    };
    status == "ok"
        && name.starts_with("test_")
        && name.contains(" (")
        && name.ends_with(')')
        && name.len() <= 1024
}

fn python_install_progress(line: &str) -> bool {
    (line.starts_with("Collecting ") && line.len() <= 512)
        || (line.starts_with("Downloading ")
            && line.len() <= 1024
            && (line.contains(".whl") || line.contains(".tar.gz")))
}

fn go_test_pass(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("--- PASS: ") else {
        return false;
    };
    let Some((name, elapsed)) = rest.split_once(" (") else {
        return false;
    };
    let Some(elapsed) = elapsed.strip_suffix(')') else {
        return false;
    };
    let Some(seconds) = elapsed.strip_suffix('s') else {
        return false;
    };
    let Some((whole, fraction)) = seconds.split_once('.') else {
        return false;
    };
    !name.is_empty()
        && name.len() <= 512
        && !name.contains(['\r', '\n'])
        && !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && !fraction.is_empty()
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

fn go_test_lifecycle(line: &str) -> bool {
    ["=== RUN ", "=== PAUSE ", "=== CONT "]
        .iter()
        .any(|prefix| {
            line.strip_prefix(prefix)
                .is_some_and(|name| !name.trim().is_empty() && name.len() <= 512)
        })
}

fn go_json_candidate(line: &[u8]) -> Result<bool, ()> {
    reject_duplicate_keys(line)?;
    let value: Value = serde_json::from_slice(line).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    const FIELDS: &[&str] = &[
        "Time",
        "Action",
        "Package",
        "Test",
        "Elapsed",
        "Output",
        "OutputType",
        "FailedBuild",
    ];
    if object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err(());
    }
    let action = object.get("Action").and_then(Value::as_str).ok_or(())?;
    if !matches!(
        action,
        "start" | "run" | "pause" | "cont" | "pass" | "bench" | "fail" | "output" | "skip"
    ) {
        return Err(());
    }
    for key in [
        "Time",
        "Package",
        "Test",
        "Output",
        "OutputType",
        "FailedBuild",
    ] {
        if object.get(key).is_some_and(|value| !value.is_string()) {
            return Err(());
        }
    }
    if object
        .get("Elapsed")
        .is_some_and(|value| !value.is_number())
    {
        return Err(());
    }
    if object
        .get("OutputType")
        .and_then(Value::as_str)
        .is_some_and(|kind| !matches!(kind, "" | "frame" | "error" | "error-continue"))
    {
        return Err(());
    }

    if action != "output" {
        return Ok(false);
    }
    let output = object.get("Output").and_then(Value::as_str).ok_or(())?;
    let Some(test) = object.get("Test").and_then(Value::as_str) else {
        return Ok(false);
    };
    if object.get("OutputType").and_then(Value::as_str) != Some("frame") {
        return Ok(false);
    }
    Ok(go_json_pass_output(output, test))
}

fn go_json_pass_output(output: &str, test: &str) -> bool {
    if test.is_empty() || test.contains(['\r', '\n']) || output.contains('\r') {
        return false;
    }
    let Some(rest) = output.strip_prefix("--- PASS: ") else {
        return false;
    };
    let Some(rest) = rest.strip_prefix(test) else {
        return false;
    };
    let Some(elapsed) = rest
        .strip_prefix(" (")
        .and_then(|rest| rest.strip_suffix(")\n"))
    else {
        return false;
    };
    let Some(seconds) = elapsed.strip_suffix('s') else {
        return false;
    };
    let Some((whole, fraction)) = seconds.split_once('.') else {
        return false;
    };
    !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && !fraction.is_empty()
        && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

impl EcosystemFilter {
    fn decide_go_json(&mut self, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        let candidate = go_json_candidate(line)?;
        self.json_confidence = self.json_confidence.saturating_add(1);
        Ok((candidate && self.json_confidence > 3).then_some(CompactKind::Passing))
    }
}

/// Detect duplicate object keys before serde_json::Value can collapse them.
fn reject_duplicate_keys(input: &[u8]) -> Result<(), ()> {
    let mut scanner = JsonScanner { input, offset: 0 };
    scanner.value(0)?;
    scanner.whitespace();
    if scanner.offset != input.len() {
        return Err(());
    }
    Ok(())
}

struct JsonScanner<'a> {
    input: &'a [u8],
    offset: usize,
}

impl JsonScanner<'_> {
    fn whitespace(&mut self) {
        while self
            .input
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<(), ()> {
        if depth > 128 {
            return Err(());
        }
        self.whitespace();
        match self.input.get(self.offset).copied().ok_or(())? {
            b'{' => self.object(depth + 1),
            b'[' => self.array(depth + 1),
            b'"' => self.string().map(|_| ()),
            _ => self.scalar(),
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), ()> {
        self.offset += 1;
        self.whitespace();
        if self.input.get(self.offset) == Some(&b'}') {
            self.offset += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            self.whitespace();
            let raw_key = self.string()?;
            let key: String = serde_json::from_slice(raw_key).map_err(|_| ())?;
            if !keys.insert(key) {
                return Err(());
            }
            self.whitespace();
            if self.input.get(self.offset) != Some(&b':') {
                return Err(());
            }
            self.offset += 1;
            self.value(depth)?;
            self.whitespace();
            match self.input.get(self.offset) {
                Some(b',') => self.offset += 1,
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(());
                }
                _ => return Err(()),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<(), ()> {
        self.offset += 1;
        self.whitespace();
        if self.input.get(self.offset) == Some(&b']') {
            self.offset += 1;
            return Ok(());
        }
        loop {
            self.value(depth)?;
            self.whitespace();
            match self.input.get(self.offset) {
                Some(b',') => self.offset += 1,
                Some(b']') => {
                    self.offset += 1;
                    return Ok(());
                }
                _ => return Err(()),
            }
        }
    }

    fn string(&mut self) -> Result<&[u8], ()> {
        if self.input.get(self.offset) != Some(&b'"') {
            return Err(());
        }
        let start = self.offset;
        self.offset += 1;
        while let Some(byte) = self.input.get(self.offset).copied() {
            self.offset += 1;
            match byte {
                b'"' => return Ok(&self.input[start..self.offset]),
                b'\\' => {
                    if self.input.get(self.offset).is_none() {
                        return Err(());
                    }
                    self.offset += 1;
                }
                0..=0x1f => return Err(()),
                _ => {}
            }
        }
        Err(())
    }

    fn scalar(&mut self) -> Result<(), ()> {
        let start = self.offset;
        while self
            .input
            .get(self.offset)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b',' | b']' | b'}'))
        {
            self.offset += 1;
        }
        if self.offset == start {
            Err(())
        } else {
            Ok(())
        }
    }
}
