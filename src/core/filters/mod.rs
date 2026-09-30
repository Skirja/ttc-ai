//! JS/TS recognizers. Every unmatched record is retained byte-for-byte.

use std::collections::{HashMap, HashSet};

use super::classification::{Family, Plan};
use super::raw_store::Stream;
use super::streaming::{CompactKind, Filter};

mod dotnet;
pub(crate) mod ecosystems;
mod jvm;
mod m8;
mod php;

pub(crate) struct JsFilter {
    plan: Plan,
    fallback_only_prefixed: bool,
    confidence: HashMap<(usize, String), [u8; 6]>,
    structured: bool,
    diagnostic_block: HashMap<(usize, String), bool>,
    disabled: bool,
}

impl JsFilter {
    pub(crate) fn new(plan: Plan) -> Self {
        let fallback_only_prefixed = plan.fallback_only_prefixed;
        Self {
            plan,
            fallback_only_prefixed,
            confidence: HashMap::new(),
            structured: false,
            diagnostic_block: HashMap::new(),
            disabled: false,
        }
    }

    fn disable(&mut self) {
        self.disabled = true;
        self.confidence.clear();
        self.diagnostic_block.clear();
    }
}

impl Filter for JsFilter {
    fn can_compact(&self) -> bool {
        !self.plan.raw && !self.disabled && self.plan.families.iter().any(is_js_family)
    }

    fn decide(&mut self, stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        self.decide_for_source(stream, line, "")
    }
}

impl JsFilter {
    fn decide_for_source(
        &mut self,
        stream: Stream,
        line: &[u8],
        source: &str,
    ) -> Result<Option<CompactKind>, ()> {
        if self.structured || self.disabled {
            return Ok(None);
        }
        let stream_index = if stream == Stream::Stdout { 0 } else { 1 };
        let key = (stream_index, source.to_owned());
        if !self.confidence.contains_key(&key) && self.confidence.len() >= 4096 {
            self.disabled = true;
            return Ok(None);
        }
        let clean = strip_ansi(line).ok_or(())?;
        let clean = clean.trim_end_matches(['\r', '\n']);
        let trim = clean.trim();
        if structured_line(trim) {
            self.structured = true;
            self.confidence.clear();
            return Ok(None);
        }
        if trim.is_empty() {
            self.confidence.entry(key).or_insert([0; 6]).fill(0);
            return Ok(None);
        }
        if protected(trim) {
            self.diagnostic_block.insert(key.clone(), true);
            self.confidence.entry(key).or_insert([0; 6]).fill(0);
            return Ok(None);
        }
        if self.diagnostic_block.get(&key).copied().unwrap_or(false)
            || self
                .diagnostic_block
                .get(&(stream_index, String::new()))
                .copied()
                .unwrap_or(false)
        {
            self.confidence.entry(key).or_insert([0; 6]).fill(0);
            return Ok(None);
        }
        if self.fallback_only_prefixed && source.is_empty() {
            self.confidence.entry(key).or_insert([0; 6]).fill(0);
            return Ok(None);
        }
        let confidence = self.confidence.entry(key.clone()).or_insert([0; 6]);
        for family in &self.plan.families {
            let Some(index) = family_index(*family) else {
                continue;
            };
            if let Some(kind) = recognize(*family, trim) {
                for (other, value) in confidence.iter_mut().enumerate() {
                    if other != index {
                        *value = 0;
                    }
                }
                confidence[index] = confidence[index].saturating_add(1);
                return Ok((confidence[index] > 3).then_some(kind));
            }
        }
        confidence.fill(0);
        Ok(None)
    }
}

fn family_index(family: Family) -> Option<usize> {
    match family {
        Family::Test => Some(0),
        Family::Lint => Some(1),
        Family::Typecheck => Some(2),
        Family::Build => Some(3),
        Family::Format => Some(4),
        Family::Install => Some(5),
        _ => None,
    }
}

fn is_js_family(family: &Family) -> bool {
    matches!(
        family,
        Family::Test
            | Family::Lint
            | Family::Typecheck
            | Family::Build
            | Family::Format
            | Family::Install
    )
}

pub(crate) fn strip_ansi(bytes: &[u8]) -> Option<String> {
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
                    | "scope"
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

pub(crate) fn protected(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if text
        .chars()
        .any(|ch| matches!(ch, '✘' | '⚠' | '↩' | '↷' | '∅' | '☠'))
    {
        return true;
    }
    if text.trim_start().starts_with("at ") {
        return true;
    }
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
        "caused by",
        " test passed",
        " tests passed",
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

/// Routes each record to the ecosystem recognizer named by the static command
/// classification. A generic test family can never activate another parser.
pub(crate) struct DispatchFilter {
    js: JsFilter,
    ecosystems: ecosystems::EcosystemFilter,
    additional: AdditionalFilter,
    sources: Vec<(String, String)>,
    source_states: HashSet<(usize, String, u8)>,
    source_limit_exceeded: bool,
}

impl DispatchFilter {
    pub(crate) fn new(plan: Plan) -> Self {
        let sources = plan.source_aliases.clone();
        Self {
            js: JsFilter::new(plan.clone()),
            ecosystems: ecosystems::EcosystemFilter::new(plan.clone()),
            additional: AdditionalFilter::new(plan),
            sources,
            source_states: HashSet::new(),
            source_limit_exceeded: false,
        }
    }
}

impl Filter for DispatchFilter {
    fn can_compact(&self) -> bool {
        !self.source_limit_exceeded
            && (self.js.can_compact()
                || self.ecosystems.can_compact()
                || self.additional.can_compact())
    }

    fn decide(&mut self, stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        if self.source_limit_exceeded {
            return Ok(None);
        }
        let (normalized, source) = unwrap_runner_prefix(line, &self.sources);
        let stream_index = if stream == Stream::Stdout { 0 } else { 1 };
        for (active, recognizer) in [
            (self.js.can_compact(), 0),
            (self.ecosystems.can_compact(), 1),
            (self.additional.can_compact(), 2),
        ] {
            if active
                && !self
                    .source_states
                    .contains(&(stream_index, source.clone(), recognizer))
            {
                if self.source_states.len() >= 4096 {
                    self.source_limit_exceeded = true;
                    self.js.disable();
                    self.ecosystems.disable();
                    self.additional.disable();
                    return Ok(None);
                }
                self.source_states
                    .insert((stream_index, source.clone(), recognizer));
            }
        }
        if self.js.can_compact()
            && let Some(kind) = self.js.decide_for_source(stream, &normalized, &source)?
        {
            return Ok(Some(kind));
        }
        self.ecosystems
            .decide_for_source(stream, &normalized, &source)
            .and_then(|decision| {
                if decision.is_some() {
                    Ok(decision)
                } else {
                    self.additional
                        .decide_for_source(stream, &normalized, &source)
                }
            })
    }
}

pub(crate) struct AdditionalFilter {
    plan: Plan,
    confidence: HashMap<(usize, String), [u8; 25]>,
    diagnostic_block: HashSet<(usize, String, u8)>,
    disabled: bool,
}

impl AdditionalFilter {
    pub(crate) fn new(plan: Plan) -> Self {
        Self {
            plan,
            confidence: HashMap::new(),
            diagnostic_block: HashSet::new(),
            disabled: false,
        }
    }

    fn disable(&mut self) {
        self.disabled = true;
        self.confidence.clear();
        self.diagnostic_block.clear();
    }

    fn can_compact(&self) -> bool {
        !self.disabled
            && !self.plan.raw
            && self.plan.families.iter().any(additional_index_is_parser)
    }

    fn decide_for_source(
        &mut self,
        stream: Stream,
        line: &[u8],
        source: &str,
    ) -> Result<Option<CompactKind>, ()> {
        if !self.can_compact() {
            return Ok(None);
        }
        let stream_index = if stream == Stream::Stdout { 0 } else { 1 };
        let key = (stream_index, source.to_owned());
        if !self.confidence.contains_key(&key) && self.confidence.len() >= 4096 {
            self.disable();
            return Ok(None);
        }
        let text = strip_ansi(line).ok_or(())?;
        let text = text.trim_end_matches(['\r', '\n']).trim();
        let confidence = self.confidence.entry(key.clone()).or_insert([0; 25]);
        if self.plan.fallback_only_prefixed && source.is_empty() {
            confidence.fill(0);
            return Ok(None);
        }
        if text.is_empty() {
            confidence.fill(0);
            return Ok(None);
        }
        if self.plan.families.contains(&Family::PhpTest)
            && (text.starts_with("Runtime: ")
                || text.starts_with("Configuration: ")
                || text.starts_with("Time: "))
        {
            confidence[0] = 0;
            return Ok(None);
        }
        if self.plan.families.contains(&Family::DotnetTest)
            && (text.starts_with("Build started ")
                || text.starts_with("Time Elapsed ")
                || dotnet_test_preamble(text))
        {
            confidence[9] = 0;
            return Ok(None);
        }
        if structured_line(text) || protected(text) {
            for family in &self.plan.families {
                if let Some(index) = additional_index(*family) {
                    confidence[index as usize] = 0;
                    self.diagnostic_block
                        .insert((stream_index, source.to_owned(), index));
                }
            }
            return Ok(None);
        }
        if self
            .plan
            .families
            .iter()
            .any(|family| m8::ctest_start(family, text))
        {
            return Ok(None);
        }
        for family in &self.plan.families {
            let Some(index) = additional_index(*family) else {
                continue;
            };
            if !additional_index_is_parser(family)
                || self
                    .diagnostic_block
                    .contains(&(stream_index, source.to_owned(), index))
                || self
                    .diagnostic_block
                    .contains(&(stream_index, String::new(), index))
            {
                continue;
            }
            let kind = additional_recognize(family, text);
            if let Some(kind) = kind {
                confidence[index as usize] =
                    confidence[index as usize].saturating_add(m8::confidence_units(family, text));
                return Ok((confidence[index as usize] > 3).then_some(kind));
            }
            confidence[index as usize] = 0;
        }
        Ok(None)
    }
}

fn dotnet_test_preamble(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("A total of ") else {
        return false;
    };
    let Some((count, remainder)) = rest.split_once(' ') else {
        return false;
    };
    count.parse::<u32>().is_ok()
        && matches!(
            remainder,
            "test files matched the specified pattern."
                | "test file matched the specified pattern."
        )
}

impl Filter for AdditionalFilter {
    fn can_compact(&self) -> bool {
        AdditionalFilter::can_compact(self)
    }

    fn decide(&mut self, stream: Stream, line: &[u8]) -> Result<Option<CompactKind>, ()> {
        self.decide_for_source(stream, line, "")
    }
}

fn additional_index(family: Family) -> Option<u8> {
    use Family::*;
    Some(match family {
        PhpTest => 0,
        PhpLint => 1,
        PhpTypecheck => 2,
        PhpFormat => 3,
        PhpInstall => 4,
        JvmTest => 5,
        JvmBuild => 6,
        JvmProgress => 7,
        JvmCompile => 8,
        DotnetTest => 9,
        DotnetBuild => 10,
        DotnetRestore => 11,
        DotnetFormat => 12,
        CmakeBuild => 13,
        Ctest => 14,
        NinjaBuild => 15,
        MakeBuild => 16,
        Rspec => 17,
        Rubocop => 18,
        RakeTest => 19,
        SwiftBuild => 20,
        SwiftTest => 21,
        ContainerBuild => 22,
        HelmLint => 23,
        TerraformValidate => 24,
        _ => return None,
    })
}

fn additional_index_is_parser(family: &Family) -> bool {
    matches!(
        family,
        Family::PhpTest
            | Family::PhpLint
            | Family::PhpInstall
            | Family::JvmTest
            | Family::JvmBuild
            | Family::JvmProgress
            | Family::DotnetTest
            | Family::DotnetBuild
            | Family::DotnetRestore
            | Family::CmakeBuild
            | Family::Ctest
            | Family::NinjaBuild
            | Family::MakeBuild
            | Family::Rspec
            | Family::Rubocop
            | Family::RakeTest
            | Family::SwiftBuild
            | Family::SwiftTest
            | Family::ContainerBuild
            | Family::HelmLint
    )
}

fn additional_recognize(family: &Family, text: &str) -> Option<CompactKind> {
    if php::recognize(family, text)
        || jvm::recognize(family, text)
        || dotnet::recognize(family, text)
    {
        Some(match family {
            Family::PhpLint
            | Family::PhpInstall
            | Family::JvmBuild
            | Family::JvmProgress
            | Family::DotnetBuild
            | Family::DotnetRestore
            | Family::CmakeBuild
            | Family::NinjaBuild
            | Family::Rubocop
            | Family::SwiftBuild
            | Family::ContainerBuild
            | Family::HelmLint => CompactKind::Progress,
            _ => CompactKind::Passing,
        })
    } else {
        m8::recognize(family, text)
    }
}

fn unwrap_runner_prefix(line: &[u8], sources: &[(String, String)]) -> (Vec<u8>, String) {
    let Ok(text) = std::str::from_utf8(line) else {
        return (line.to_vec(), String::new());
    };
    let ending_len = if text.ends_with("\r\n") {
        2
    } else if text.ends_with('\n') {
        1
    } else {
        0
    };
    let body_end = text.len().saturating_sub(ending_len);
    let body = &text[..body_end];
    for (alias, source) in sources {
        let prefixes = [
            format!("[{alias}] "),
            format!("[{alias}]: "),
            format!("{alias} | "),
        ];
        if let Some(prefix) = prefixes
            .iter()
            .find(|prefix| body.starts_with(prefix.as_str()))
        {
            let mut normalized = body.as_bytes()[prefix.len()..].to_vec();
            normalized.extend_from_slice(&line[body_end..]);
            return (normalized, source.clone());
        }
        if let Some(rest) = body
            .strip_prefix(alias)
            .and_then(|rest| rest.strip_prefix('#'))
            && let Some((task, payload)) = rest.split_once(": ")
            && matches!(
                task,
                "test" | "e2e" | "lint" | "typecheck" | "build" | "format"
            )
        {
            let payload = payload.trim_start_matches([' ', '\t']);
            let mut normalized = payload.as_bytes().to_vec();
            normalized.extend_from_slice(&line[body_end..]);
            return (normalized, format!("{source}\0{task}"));
        }
        if let Some(rest) = body
            .strip_prefix(alias)
            .and_then(|rest| rest.strip_prefix(':'))
        {
            if let Some((task, payload)) = rest.split_once(" | ")
                && matches!(
                    task,
                    "test"
                        | "e2e"
                        | "lint"
                        | "typecheck"
                        | "type-check"
                        | "build"
                        | "format"
                        | "fmt"
                )
            {
                let payload = payload.trim_start_matches([' ', '\t']);
                let mut normalized = payload.as_bytes().to_vec();
                normalized.extend_from_slice(&line[body_end..]);
                return (normalized, format!("{source}\0{task}"));
            }
            if let Some((task, payload)) = rest.split_once(": ")
                && matches!(
                    task,
                    "test"
                        | "e2e"
                        | "lint"
                        | "typecheck"
                        | "type-check"
                        | "build"
                        | "format"
                        | "fmt"
                )
            {
                let mut normalized = payload.as_bytes().to_vec();
                normalized.extend_from_slice(&line[body_end..]);
                return (normalized, format!("{source}\0{task}"));
            }
            if let Some(payload) = rest.strip_prefix([' ', '\t']) {
                let payload = payload.trim_start_matches([' ', '\t']);
                let mut normalized = payload.as_bytes().to_vec();
                normalized.extend_from_slice(&line[body_end..]);
                return (normalized, source.clone());
            }
        }
        if let Some(rest) = body
            .strip_prefix(alias)
            .and_then(|rest| rest.strip_prefix(' '))
            && let Some((task, payload)) = rest.split_once(": ")
        {
            let task = task.trim();
            if matches!(
                task,
                "test" | "e2e" | "lint" | "typecheck" | "type-check" | "build" | "format" | "fmt"
            ) {
                let payload = payload.trim_start_matches([' ', '\t']);
                let mut normalized = payload.as_bytes().to_vec();
                normalized.extend_from_slice(&line[body_end..]);
                return (normalized, format!("{source}\0{task}"));
            }
        }
    }
    (line.to_vec(), String::new())
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
        _ => None,
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
