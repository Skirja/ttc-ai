//! Independently authored recognizers for complete, noise-only records.
use regex::Regex;
use std::sync::OnceLock;
#[derive(Clone, Copy)]
pub enum Kind {
    Passing,
    Progress,
}
pub struct Rule {
    pub family: &'static str,
    pub pattern: &'static str,
    pub example: &'static str,
    pub kind: Kind,
}
pub const RULES: &[Rule] = &[
    Rule {
        family: "cargo-test",
        pattern: r"^test [^\r\n]+ \.\.\. ok$",
        example: "test auth::login ... ok",
        kind: Kind::Passing,
    },
    Rule {
        family: "go-test",
        pattern: r"^--- PASS: [^\r\n]+ \([0-9.]+s\)$",
        example: "--- PASS: TestLogin (0.00s)",
        kind: Kind::Passing,
    },
    Rule {
        family: "pytest",
        pattern: r"^\S+::\S+ PASSED(?:\s+\[\s*[0-9]+%\])?$",
        example: "tests/test_auth.py::test_login PASSED [ 50%]",
        kind: Kind::Passing,
    },
    Rule {
        family: "js-test",
        pattern: r"^\s*[✓✔] [^\r\n]+(?:\([0-9]+ tests?\)|[0-9]+ms)\s*$",
        example: " ✓ tests/auth.test.ts (12 tests)",
        kind: Kind::Passing,
    },
    Rule {
        family: "bun-test",
        pattern: r"^\(pass\) [^\r\n]+ \[[0-9.]+ms\]$",
        example: "(pass) auth > login [0.42ms]",
        kind: Kind::Passing,
    },
    Rule {
        family: "dotnet",
        pattern: r"^\s*Passed [^\r\n]+ \[[0-9.]+ (?:ms|s)\]$",
        example: "  Passed App.Tests.Login [12 ms]",
        kind: Kind::Passing,
    },
    Rule {
        family: "rust",
        pattern: r"^\s+(?:Compiling|Checking) [A-Za-z0-9_.-]+ v[0-9][A-Za-z0-9.+_-]*(?: \([^\r\n]+\))?$",
        example: "   Compiling serde v1.0.0",
        kind: Kind::Progress,
    },
    Rule {
        family: "cargo-test",
        pattern: r"^\s+(?:Compiling|Checking) [A-Za-z0-9_.-]+ v[0-9][A-Za-z0-9.+_-]*(?: \([^\r\n]+\))?$",
        example: "   Compiling serde v1.0.0",
        kind: Kind::Progress,
    },
    Rule {
        family: "jvm",
        pattern: r"^(?:Downloading|Downloaded) from [A-Za-z0-9_.-]+: https?://\S+(?: \([0-9.]+ [A-Za-z/ ,]+\))?$",
        example: "Downloading from central: https://repo.maven.apache.org/maven2/pkg.jar",
        kind: Kind::Progress,
    },
    Rule {
        family: "native-build",
        pattern: r"^\[[0-9]+/[0-9]+\] Building (?:C|CXX) object \S+$",
        example: "[2/100] Building CXX object src/main.cpp.o",
        kind: Kind::Progress,
    },
    Rule {
        family: "native-build",
        pattern: r"^\[\s*[0-9]+%\] Building (?:C|CXX) object \S+$",
        example: "[ 12%] Building C object src/main.c.o",
        kind: Kind::Progress,
    },
    Rule {
        family: "js-build",
        pattern: r"^(?:transforming\.\.\.|rendering chunks\.\.\.|computing gzip size\.\.\.)$",
        example: "transforming...",
        kind: Kind::Progress,
    },
    Rule {
        family: "container",
        pattern: r"^#[0-9]+ (?:DONE [0-9.]+s|CACHED)$",
        example: "#12 DONE 0.2s",
        kind: Kind::Progress,
    },
    Rule {
        family: "container",
        pattern: r"^[a-f0-9]{12}: (?:Already exists|Pull complete|Download complete|Verifying Checksum|Waiting)$",
        example: "abcdef012345: Pull complete",
        kind: Kind::Progress,
    },
    Rule {
        family: "git",
        pattern: r"^(?:remote: )?(?:Counting|Compressing|Receiving|Resolving) objects: [0-9]+% \([0-9]+/[0-9]+\)(?:, done\.)?$",
        example: "remote: Counting objects: 100% (10/10), done.",
        kind: Kind::Progress,
    },
    Rule {
        family: "packages",
        pattern: r"^Progress: resolved [0-9]+, reused [0-9]+, downloaded [0-9]+, added [0-9]+(?:, done)?$",
        example: "Progress: resolved 100, reused 80, downloaded 20, added 100, done",
        kind: Kind::Progress,
    },
    Rule {
        family: "packages",
        pattern: r"^\s*Downloading \S+ \([0-9.]+ (?:kB|MB)\)$",
        example: "Downloading example.whl (12.0 kB)",
        kind: Kind::Progress,
    },
    Rule {
        family: "php-test",
        pattern: r"^\s*[.]+\s+[0-9]+ / [0-9]+ \(\s*[0-9]+%\)$",
        example: ".......... 10 / 100 ( 10%)",
        kind: Kind::Progress,
    },
    Rule {
        family: "php-lint",
        pattern: r"^\s*[0-9]+/[0-9]+ \[[=> -]+\]\s+[0-9]+%$",
        example: " 10/10 [====================] 100%",
        kind: Kind::Progress,
    },
    Rule {
        family: "ruby-test",
        pattern: r"^[.]{10,}$",
        example: "....................",
        kind: Kind::Progress,
    },
    Rule {
        family: "ruby-lint",
        pattern: r"^Inspecting [0-9]+ files?$",
        example: "Inspecting 20 files",
        kind: Kind::Progress,
    },
    Rule {
        family: "python-task",
        pattern: r"^[A-Za-z0-9_-]+: (?:install_deps|install_package)> \S+(?: [^\r\n]+)?$",
        example: "py312: install_deps> python -I -m pip install pytest",
        kind: Kind::Progress,
    },
    Rule {
        family: "other-build",
        pattern: r"^\[[0-9]+/[0-9]+\] (?:Compiling|Emitting module) [A-Za-z0-9_. /-]+$",
        example: "[4/20] Compiling App main.swift",
        kind: Kind::Progress,
    },
];
pub fn match_line(families: &[String], line: &str) -> Option<Kind> {
    // A progress-looking record containing diagnostic language is never noise.
    let lower = line.to_lowercase();
    if [
        "error",
        "warn",
        "fail",
        "panic",
        "fatal",
        "vulnerab",
        "security",
        "deprecated",
        "assert",
        "expected:",
        "actual:",
    ]
    .iter()
    .any(|s| lower.contains(s))
    {
        return None;
    }
    static COMPILED: OnceLock<Vec<Regex>> = OnceLock::new();
    let compiled = COMPILED.get_or_init(|| {
        RULES
            .iter()
            .map(|r| Regex::new(r.pattern).expect("static semantic rule"))
            .collect()
    });
    RULES
        .iter()
        .zip(compiled)
        .find(|(rule, re)| families.iter().any(|f| f == rule.family) && re.is_match(line))
        .map(|(r, _)| r.kind)
}
