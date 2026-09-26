//! JS/TS recognizers. Every unmatched record is retained byte-for-byte.

use super::classification::{Family, Plan};
use super::raw_store::Stream;
use super::streaming::{CompactKind, Filter};

pub(crate) struct JsFilter {
    plan: Plan,
    confidence: [[u8; 6]; 2],
    structured: bool,
    diagnostic_block: [bool; 2],
}

impl JsFilter {
    pub(crate) fn new(plan: Plan) -> Self {
        Self {
            plan,
            confidence: [[0; 6]; 2],
            structured: false,
            diagnostic_block: [false; 2],
        }
    }
}

impl Filter for JsFilter {
    fn can_compact(&self) -> bool {
        !self.plan.raw && !self.plan.families.is_empty()
    }

    fn decide(&mut self, stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        if self.structured {
            return Ok(None);
        }
        let stream_index = if stream == Stream::Stdout { 0 } else { 1 };
        let clean = strip_ansi(line).ok_or(())?;
        let clean = clean.trim_end_matches(['\r', '\n']);
        let trim = clean.trim();
        if structured_line(trim) {
            self.structured = true;
            self.confidence[stream_index].fill(0);
            return Ok(None);
        }
        if trim.is_empty() {
            self.diagnostic_block[stream_index] = false;
            self.confidence[stream_index].fill(0);
            return Ok(None);
        }
        if protected(trim) {
            self.diagnostic_block[stream_index] = true;
            self.confidence[stream_index].fill(0);
            return Ok(None);
        }
        if self.diagnostic_block[stream_index] {
            self.confidence[stream_index].fill(0);
            return Ok(None);
        }
        for family in &self.plan.families {
            if let Some(kind) = recognize(*family, trim) {
                let index = family_index(*family);
                for (other, value) in self.confidence[stream_index].iter_mut().enumerate() {
                    if other != index {
                        *value = 0;
                    }
                }
                self.confidence[stream_index][index] =
                    self.confidence[stream_index][index].saturating_add(1);
                return Ok((self.confidence[stream_index][index] > 3).then_some(kind));
            }
        }
        self.confidence[stream_index].fill(0);
        Ok(None)
    }
}

fn family_index(family: Family) -> usize {
    match family {
        Family::Test => 0,
        Family::Lint => 1,
        Family::Typecheck => 2,
        Family::Build => 3,
        Family::Format => 4,
        Family::Install => 5,
    }
}

fn strip_ansi(bytes: &[u8]) -> Option<String> {
    let input = std::str::from_utf8(bytes).ok()?;
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.next()? != '[' {
                return None;
            }
            let mut closed = false;
            for code in chars.by_ref() {
                if code == 'm' {
                    closed = true;
                    break;
                }
                if !code.is_ascii_digit() && code != ';' {
                    return None;
                }
            }
            if !closed {
                return None;
            }
        } else if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            return None;
        } else {
            result.push(ch);
        }
    }
    Some(result)
}

fn structured_line(text: &str) -> bool {
    text.starts_with('{')
        || text.starts_with("<?xml")
        || text.starts_with("<testsuite")
        || text.starts_with("---")
        || text.starts_with("TAP version")
        || text.starts_with("not ok ")
        || text.starts_with("ok ")
        || text.starts_with("1..")
        || text == "["
        || text.starts_with("[{")
        || text.starts_with("[\"")
        || text.split_once(": ").is_some_and(|(key, _)| {
            !matches!(
                key.to_ascii_lowercase().as_str(),
                "progress"
                    | "tests"
                    | "time"
                    | "warning"
                    | "warn"
                    | "error"
                    | "failure"
                    | "fatal"
                    | "security"
                    | "deprecated"
            ) && !key.is_empty()
                && key.len() <= 64
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

fn protected(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if [
        "error",
        "warn",
        "fail",
        "panic",
        "fatal",
        "assert",
        "expected",
        "actual",
        "deprecat",
        "vulnerab",
        "security",
        "stack trace",
        "snapshot",
        "diff",
        "exception",
        "traceback",
        "at ",
        "caused by",
        " passed",
        "test suites",
        "test files",
        "summary:",
    ]
    .iter()
    .any(|part| lower.contains(part))
    {
        return true;
    }
    // Conservatively retain any location-like :line[:column], including
    // extensionless file names such as Makefile:42.
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b':' && index > 0 && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            return true;
        }
    }
    false
}

fn recognize(family: Family, text: &str) -> Option<CompactKind> {
    match family {
        Family::Test => passing_test(text).then_some(CompactKind::Passing),
        Family::Lint => prefixed_progress(
            text,
            &[
                "[eslint] Checking ",
                "[biome] Checked ",
                "[oxlint] Checked ",
                "[stylelint] Checking ",
            ],
        )
        .then_some(CompactKind::Progress),
        Family::Typecheck => prefixed_progress(
            text,
            &[
                "[tsc] Checking ",
                "[vue-tsc] Checking ",
                "[svelte-check] Checking ",
                "[flow] Checking ",
            ],
        )
        .then_some(CompactKind::Progress),
        Family::Build => (text.starts_with("transforming (")
            && text.ends_with(')')
            && text[14..text.len() - 1].bytes().all(|x| x.is_ascii_digit())
            || prefixed_progress(
                text,
                &[
                    "[vite] transforming ",
                    "[next] compiling ",
                    "[nuxt] building ",
                    "[webpack] building ",
                    "[rollup] building ",
                    "[esbuild] building ",
                    "[tsup] building ",
                    "[swc] building ",
                ],
            ))
        .then_some(CompactKind::Progress),
        Family::Format => prefixed_progress(
            text,
            &[
                "[prettier] Checking ",
                "[biome] Formatting ",
                "[dprint] Checking ",
            ],
        )
        .then_some(CompactKind::Progress),
        Family::Install => (text.starts_with("Progress: resolved ")
            && text.contains(", reused ")
            && text.contains(", downloaded ")
            || prefixed_progress(
                text,
                &["[npm] fetching ", "[yarn] fetching ", "[bun] fetching "],
            ))
        .then_some(CompactKind::Progress),
    }
}

fn prefixed_progress(text: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| {
        text.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.len() <= 512)
    })
}

fn passing_test(text: &str) -> bool {
    if let Some(rest) = text.strip_prefix("PASS ") {
        return rest.len() <= 512 && (rest.contains(".test.") || rest.contains(".spec."));
    }
    for mark in ["✓ ", "✔ ", "√ "] {
        if let Some(rest) = text.strip_prefix(mark) {
            return !rest.is_empty() && rest.len() <= 512 && !rest.ends_with(" tests");
        }
    }
    false
}
