//! Parse complete location-bearing diagnostics before deduplicating identical records.
use regex::Regex;
use std::{collections::HashSet, sync::OnceLock};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub line: u64,
    pub column: Option<u64>,
    pub severity: String,
    pub code: Option<String>,
    pub message: String,
}
pub fn parse(s: &str) -> Option<Diagnostic> {
    static PAREN: OnceLock<Regex> = OnceLock::new();
    static COLON: OnceLock<Regex> = OnceLock::new();
    let paren = PAREN.get_or_init(|| {
        Regex::new(r"^(.+)\(([0-9]+),([0-9]+)\): (error|warning) ([A-Z]+[0-9]+): (.+)$")
            .expect("diagnostic grammar")
    });
    if let Some(c) = paren.captures(s) {
        return Some(Diagnostic {
            file: c[1].into(),
            line: c[2].parse().ok()?,
            column: Some(c[3].parse().ok()?),
            severity: c[4].into(),
            code: Some(c[5].into()),
            message: c[6].into(),
        });
    }
    let colon = COLON.get_or_init(|| {
        Regex::new(r"^(.+?):([0-9]+)(?::([0-9]+))?: (error|warning|fatal error|note): (.+)$")
            .expect("diagnostic grammar")
    });
    let c = colon.captures(s)?;
    Some(Diagnostic {
        file: c[1].into(),
        line: c[2].parse().ok()?,
        column: c.get(3).and_then(|s| s.as_str().parse().ok()),
        severity: c[4].into(),
        code: None,
        message: c[5].into(),
    })
}
#[derive(Default)]
pub struct Deduplicator {
    seen: HashSet<(usize, String)>,
}
impl Deduplicator {
    pub fn duplicate(&mut self, families: &[String], channel: usize, s: &str) -> bool {
        if s.len() > 4096
            || !families.iter().any(|s| {
                [
                    "typescript",
                    "python-lint",
                    "compiler",
                    "dotnet",
                    "go",
                    "rust",
                    "js-lint",
                    "php-lint",
                ]
                .contains(&s.as_str())
            })
            || parse(s).is_none()
        {
            return false;
        }
        let key = (channel, s.to_owned());
        if self.seen.contains(&key) {
            return true;
        }
        if self.seen.len() < 1024 {
            self.seen.insert(key);
        }
        false
    }
}
