//! Conservative, read-only hints for output filtering.

use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Test,
    Lint,
    Typecheck,
    Build,
    Format,
    Install,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ManifestHints {
    pub scripts: Vec<(String, String)>,
}

impl ManifestHints {
    pub fn load(directory: &Path) -> Option<Self> {
        let mut contents = Vec::new();
        fs::File::open(directory.join("package.json"))
            .ok()?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut contents)
            .ok()?;
        if contents.len() > 1024 * 1024 {
            return None;
        }
        let value: serde_json::Value = serde_json::from_slice(&contents).ok()?;
        let scripts = value.get("scripts")?.as_object()?;
        let scripts = scripts
            .iter()
            .map(|(name, body)| Some((name.clone(), body.as_str()?.to_owned())))
            .collect::<Option<Vec<_>>>()?;
        Some(Self { scripts })
    }
    fn script(&self, name: &str) -> Option<&str> {
        self.scripts
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    pub families: Vec<Family>,
    pub raw: bool,
}

impl Plan {
    fn add(&mut self, family: Family) {
        if !self.families.contains(&family) {
            self.families.push(family);
        }
    }
}

pub(crate) fn classify(arguments: &[OsString], cwd: &Path, hints: Option<&ManifestHints>) -> Plan {
    let words = if arguments.len() == 1 {
        let Some(command) = arguments[0].to_str() else {
            return Plan::default();
        };
        let Some(words) = split_shell(command) else {
            return Plan::default();
        };
        words
    } else {
        let Some(words) = arguments
            .iter()
            .map(|word| word.to_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
        else {
            return Plan::default();
        };
        words
    };
    let mut plan = Plan::default();
    if machine_output(&words) {
        plan.raw = true;
        return plan;
    }
    let mut directory = cwd.to_path_buf();
    let mut segment = Vec::new();
    for word in words.into_iter().chain(std::iter::once("&&".to_owned())) {
        if word == "&&" || word == ";" {
            classify_segment(
                &segment,
                &mut directory,
                cwd,
                hints,
                &mut plan,
                0,
                &mut Vec::new(),
            );
            segment.clear();
        } else if matches!(word.as_str(), "|" | "||" | ">" | "<" | "&") {
            return Plan::default();
        } else {
            segment.push(word);
        }
    }
    plan
}

fn machine_output(words: &[String]) -> bool {
    words.iter().any(|word| machine_flag(word))
        || words.windows(2).any(|pair| {
            matches!(pair[0].as_str(), "--reporter" | "--format")
                && matches!(
                    pair[1].as_str(),
                    "json" | "jsonl" | "xml" | "yaml" | "sarif" | "tap"
                )
        })
}

fn machine_flag(word: &str) -> bool {
    matches!(
        word,
        "--json" | "--jsonl" | "--xml" | "--yaml" | "--sarif" | "--output" | "-o" | "--format"
    ) || [
        "--json=",
        "--jsonl=",
        "--xml=",
        "--yaml=",
        "--sarif=",
        "--output=",
        "--format=",
        "--reporter=json",
        "--reporter=xml",
        "--reporter=tap",
    ]
    .iter()
    .any(|prefix| word.starts_with(prefix))
}

fn workspace_selector(word: &str) -> bool {
    matches!(
        word,
        "--workspace" | "--workspaces" | "-w" | "--filter" | "-r" | "--recursive" | "foreach"
    ) || word.starts_with("--workspace=")
        || word.starts_with("--filter=")
        || word.starts_with("--recursive=")
        || (word.starts_with("-w") && word.len() > 2 && !word.starts_with("--"))
}

fn classify_segment(
    words: &[String],
    directory: &mut PathBuf,
    root: &Path,
    hints: Option<&ManifestHints>,
    plan: &mut Plan,
    depth: usize,
    seen: &mut Vec<String>,
) {
    if depth >= 16 || words.is_empty() {
        return;
    }
    let mut words = words;
    while words.first().is_some_and(|word| is_assignment(word)) {
        words = &words[1..];
    }
    let Some(program) = words.first() else { return };
    if program == "cd" {
        if let Some(path) = words.get(1) {
            *directory = directory.join(path);
        }
        return;
    }
    let mut name = Path::new(program)
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or(program);
    let mut args = &words[1..];
    if name == "env" {
        while args.first().is_some_and(|word| is_assignment(word)) {
            args = &args[1..];
        }
        let Some(nested) = args.first() else { return };
        name = Path::new(nested)
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or(nested);
        args = &args[1..];
    }
    if matches!(name, "npx" | "pnpx" | "bunx")
        || matches!(name, "pnpm" | "yarn" | "bun" | "npm")
            && args
                .first()
                .is_some_and(|x| matches!(x.as_str(), "exec" | "dlx"))
    {
        args = if matches!(args.first().map(String::as_str), Some("exec" | "dlx")) {
            &args[1..]
        } else {
            args
        };
        while args.first().is_some_and(|x| x.starts_with('-')) {
            args = &args[1..];
        }
        if let Some(tool) = args.first() {
            classify_tool(
                Path::new(tool)
                    .file_name()
                    .and_then(|x| x.to_str())
                    .unwrap_or(tool),
                &args[1..],
                plan,
            );
        }
        return;
    }
    if matches!(name, "npm" | "pnpm" | "yarn" | "bun") {
        if args.iter().any(|x| x == "dev" || x == "watch") {
            plan.raw = true;
            return;
        }
        let mut workspace = args.iter().any(|x| workspace_selector(x));
        let mut rest = args;
        while !rest.is_empty() {
            if matches!(
                rest[0].as_str(),
                "-C" | "--dir" | "--filter" | "--workspace" | "-w"
            ) && rest.len() > 1
            {
                if matches!(rest[0].as_str(), "-C" | "--dir") {
                    *directory = directory.join(&rest[1]);
                }
                rest = &rest[2..];
            } else if rest[0].starts_with('-') {
                rest = &rest[1..];
            } else {
                break;
            }
        }
        if rest.first().is_some_and(|x| x == "workspace") && rest.len() >= 3 {
            workspace = true;
            rest = &rest[2..];
        } else if rest.first().is_some_and(|x| x == "workspaces")
            && rest.get(1).is_some_and(|x| x == "foreach")
        {
            workspace = true;
            rest = &rest[2..];
            while rest.first().is_some_and(|x| x.starts_with('-')) {
                rest = &rest[1..];
            }
        }
        if workspace {
            // M6 resolves the scripts of selected and recursive workspaces.
            // The root manifest cannot establish which tool will actually run.
            plan.raw = true;
            return;
        }
        if rest
            .first()
            .is_some_and(|x| matches!(x.as_str(), "ci" | "install"))
        {
            plan.add(Family::Install);
            return;
        }
        if rest.first().is_some_and(|x| x == "test") && name == "bun" {
            plan.add(Family::Test);
            return;
        }
        if rest.first().is_some_and(|x| x == "run") {
            rest = &rest[1..];
        }
        let Some(script) = rest.first() else { return };
        let local_hints = if directory == root && hints.is_some() {
            None
        } else {
            ManifestHints::load(directory)
        };
        let selected_hints = if directory == root {
            hints.or(local_hints.as_ref())
        } else {
            local_hints.as_ref()
        };
        if selected_hints.is_none()
            && !matches!(fs::metadata(directory.join("package.json")), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        {
            plan.raw = true;
            return;
        }
        if let Some(body) = selected_hints.and_then(|manifest| manifest.script(script)) {
            if seen.contains(script) {
                return;
            }
            seen.push(script.to_owned());
            if let Some(body_words) = split_shell(body) {
                if machine_output(&body_words) {
                    plan.raw = true;
                } else {
                    let mut part = Vec::new();
                    for word in body_words
                        .into_iter()
                        .chain(std::iter::once("&&".to_owned()))
                    {
                        if matches!(word.as_str(), "&&" | ";") {
                            classify_segment(
                                &part,
                                directory,
                                root,
                                selected_hints,
                                plan,
                                depth + 1,
                                seen,
                            );
                            part.clear();
                        } else if matches!(word.as_str(), "|" | "||" | ">" | "<" | "&") {
                            plan.raw = true;
                            break;
                        } else {
                            part.push(word);
                        }
                    }
                }
            } else {
                plan.raw = true;
            }
            seen.pop();
        }
        return;
    }
    classify_tool(name, args, plan);
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn classify_tool(name: &str, args: &[String], plan: &mut Plan) {
    if args.iter().any(|x| x == "dev" || x == "watch") {
        plan.raw = true;
        return;
    }
    let family = match name {
        "vitest" | "jest" | "mocha" | "ava" | "tap" => Some(Family::Test),
        "playwright" if args.first().is_some_and(|x| x == "test") => Some(Family::Test),
        "cypress" if args.first().is_some_and(|x| x == "run") => Some(Family::Test),
        "eslint" | "oxlint" | "stylelint" => Some(Family::Lint),
        "biome"
            if args
                .first()
                .is_some_and(|x| matches!(x.as_str(), "check" | "lint")) =>
        {
            Some(Family::Lint)
        }
        "biome" if args.first().is_some_and(|x| x == "format") => Some(Family::Format),
        "tsc" | "vue-tsc" | "svelte-check" | "flow" => Some(Family::Typecheck),
        "vite" | "next" | "nuxt" if args.first().is_some_and(|x| x == "build") => {
            Some(Family::Build)
        }
        "webpack" | "rollup" | "esbuild" | "tsup" | "swc" => Some(Family::Build),
        "prettier" if args.iter().any(|x| x == "--check") => Some(Family::Format),
        "dprint" if args.first().is_some_and(|x| x == "check") => Some(Family::Format),
        _ => None,
    };
    if let Some(family) = family {
        plan.add(family);
    }
}

/// Tokenizes only shell syntax whose command boundaries are unambiguous.
fn split_shell(command: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => word.push(chars.next()?),
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, '\\') => word.push(chars.next()?),
            (None, ' ' | '\t' | '\n') => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            (None, '&' | '|' | ';' | '<' | '>') => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                let mut op = ch.to_string();
                if chars.peek() == Some(&ch) {
                    op.push(chars.next()?);
                }
                words.push(op);
            }
            (None, '$' | '`') => return None,
            (None, c) => word.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    if !word.is_empty() {
        words.push(word);
    }
    Some(words)
}
