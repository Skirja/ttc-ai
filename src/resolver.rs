//! Bounded, non-executing workload discovery. Script names never identify tools.
use crate::command::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

type ManifestCache = HashMap<PathBuf, (Vec<u8>, Value)>;
static JSON_CACHE: OnceLock<Mutex<ManifestCache>> = OnceLock::new();
fn json(path: &Path) -> Option<Value> {
    if std::fs::metadata(path).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let hash = Sha256::digest(&bytes).to_vec();
    let mut cache = JSON_CACHE.get_or_init(Default::default).lock().ok()?;
    if let Some((old, v)) = cache.get(path)
        && old == &hash
    {
        return Some(v.clone());
    }
    let v = serde_json::from_slice(&bytes).ok()?;
    if cache.len() > 512 {
        cache.clear();
    }
    cache.insert(path.into(), (hash, v));
    cache.get(path).map(|(_, v)| v.clone())
}
fn node(label: &str, cwd: &Path) -> WorkloadGraph {
    WorkloadGraph {
        label: label.into(),
        cwd: cwd.into(),
        family: None,
        confidence: Confidence::Unknown,
        children: vec![],
        notes: vec![],
    }
}
fn project(cwd: &Path, file: &str) -> Option<PathBuf> {
    cwd.ancestors()
        .find(|p| p.join(file).exists())
        .map(Path::to_path_buf)
}
fn workspaces(cwd: &Path) -> Vec<PathBuf> {
    let root = cwd
        .ancestors()
        .find(|p| {
            p.join("pnpm-workspace.yaml").exists()
                || json(&p.join("package.json")).is_some_and(|v| v.get("workspaces").is_some())
        })
        .unwrap_or(cwd);
    let mut patterns: Vec<String> = vec![];
    if let Some(v) = json(&root.join("package.json")) {
        let w = &v["workspaces"];
        if let Some(a) = w.as_array().or_else(|| w["packages"].as_array()) {
            patterns.extend(a.iter().filter_map(Value::as_str).map(str::to_owned));
        }
    }
    if let Ok(s) = std::fs::read_to_string(root.join("pnpm-workspace.yaml"))
        && let Ok(v) = serde_yaml::from_str::<Value>(&s)
        && let Some(a) = v["packages"].as_array()
    {
        patterns.extend(a.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    let mut dirs = BTreeSet::new();
    dirs.insert(root.to_path_buf());
    for p in patterns.iter().filter(|p| !p.starts_with('!')) {
        if let Ok(paths) = glob::glob(&root.join(p).join("package.json").to_string_lossy()) {
            for path in paths.flatten().take(4096) {
                if let Some(parent) = path.parent() {
                    dirs.insert(parent.into());
                }
            }
        }
    }
    dirs.into_iter()
        .filter(|p| {
            !patterns
                .iter()
                .filter_map(|p| p.strip_prefix('!'))
                .any(|s| {
                    glob::Pattern::new(s)
                        .is_ok_and(|pat| pat.matches_path(p.strip_prefix(root).unwrap_or(p)))
                })
        })
        .collect()
}
#[derive(Default)]
struct Resolver {
    active: HashSet<String>,
    nodes: usize,
}
impl Resolver {
    fn script(&mut self, pm: &str, script: &str, cwd: &Path, depth: usize) -> WorkloadGraph {
        let mut n = node(&format!("{pm} script {script}"), cwd);
        let manifest = if pm == "composer" {
            "composer.json"
        } else {
            "package.json"
        };
        let Some(dir) = project(cwd, manifest) else {
            n.notes.push("manifest unavailable".into());
            return n;
        };
        n.cwd = dir.clone();
        let Some(v) = json(&dir.join(manifest)) else {
            return n;
        };
        let body = &v["scripts"][script];
        if body.is_null() {
            n.notes.push("script not defined".into());
            return n;
        }
        let key = format!("{}:{pm}:{script}", dir.display());
        if !self.active.insert(key.clone()) {
            n.notes.push("script cycle".into());
            return n;
        }
        for name in [
            format!("pre{script}"),
            script.into(),
            format!("post{script}"),
        ] {
            let b = &v["scripts"][&name];
            // Lifecycle execution differs by manager/version: include possible hooks conservatively.
            let bodies: Vec<&str> = match b {
                Value::String(s) => vec![s],
                Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                _ => vec![],
            };
            for body in bodies {
                if pm == "composer" && body.starts_with('@') {
                    if let Ok(a) = shell_words::split(&body[1..])
                        && let Some(s) = a.first()
                    {
                        n.children.push(self.script(pm, s, &dir, depth + 1));
                    }
                } else {
                    n.children.push(self.text(body, &dir, depth + 1));
                }
            }
        }
        self.active.remove(&key);
        n.confidence = Confidence::ResolvedProjectScript;
        n
    }
    fn text(&mut self, s: &str, cwd: &Path, depth: usize) -> WorkloadGraph {
        let ast = parse(s, Shell::Posix);
        let mut n = node(s, cwd);
        if depth >= 32 || self.nodes >= 4096 {
            n.notes.push("resolution limit".into());
            return n;
        }
        self.nodes += 1;
        for c in &ast.commands {
            n.children.push(self.argv(&c.argv, cwd, depth + 1));
        }
        if ast.uncertain {
            n.notes.push("dynamic or unsupported shell syntax".into());
        }
        if !ast.uncertain && !n.children.is_empty() {
            n.confidence = Confidence::ResolvedNestedWrapper;
        }
        n
    }
    fn argv(&mut self, a: &[String], cwd: &Path, depth: usize) -> WorkloadGraph {
        let mut n = node(&a.join(" "), cwd);
        if a.is_empty() || depth >= 32 || self.nodes >= 4096 {
            return n;
        }
        self.nodes += 1;
        if a[0].split_once('=').is_some_and(|(name, _)| {
            !name.is_empty()
                && name.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
        }) {
            n.children.push(self.argv(&a[1..], cwd, depth + 1));
            n.confidence = Confidence::ResolvedNestedWrapper;
            return n;
        }
        let exe = Path::new(&a[0])
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let exe = exe.strip_suffix(".exe").unwrap_or(&exe);
        let sub = a.get(1).map(String::as_str).unwrap_or("");
        if ["sudo", "xargs"].contains(&exe) {
            let mut i = 1;
            while i < a.len() && a[i].starts_with('-') {
                if a[i] == "--" {
                    i += 1;
                    break;
                }
                let value_flags = if exe == "sudo" {
                    vec!["-u", "-g", "--user", "--group"]
                } else {
                    vec!["-n", "-P", "-I", "--max-args", "--max-procs", "--replace"]
                };
                if value_flags.contains(&a[i].as_str()) {
                    i += 2;
                } else if ["-0", "-r", "--no-run-if-empty", "-n", "-H", "-E"]
                    .contains(&a[i].as_str())
                {
                    i += 1;
                } else {
                    return n;
                }
            }
            if i < a.len() {
                n.children.push(self.argv(&a[i..], cwd, depth + 1));
            }
            n.notes
                .push("privileged or input-dependent wrapper; passthrough".into());
            return n;
        }
        if ["make", "just", "task"].contains(&exe) {
            if let Some(bodies) = crate::project::task_commands(exe, sub, cwd) {
                for body in bodies {
                    n.children.push(self.text(&body, cwd, depth + 1));
                }
                n.confidence = Confidence::ResolvedProjectScript;
                return n;
            }
            n.notes
                .push("dynamic or unavailable task definition".into());
            return n;
        }
        if [
            "command",
            "env",
            "cross-env",
            "timeout",
            "uv",
            "poetry",
            "bundle",
        ]
        .contains(&exe)
        {
            let start = match exe {
                "uv" | "poetry" => a.iter().position(|s| s == "run").map(|i| i + 1),
                "bundle" => a.iter().position(|s| s == "exec").map(|i| i + 1),
                "timeout" => Some(2),
                _ => Some(1),
            };
            if let Some(mut i) = start {
                while i < a.len() && (a[i].contains('=') || a[i] == "--") {
                    i += 1;
                }
                if i < a.len() && !a[i].starts_with('-') {
                    n.children.push(self.argv(&a[i..], cwd, depth + 1));
                    n.confidence = Confidence::ResolvedNestedWrapper;
                    return n;
                }
            }
        }
        if (exe == "python" || exe == "python3") && sub == "-m" {
            return self.argv(&a[2..], cwd, depth + 1);
        }
        if ["npx", "pnpx", "bunx"].contains(&exe) {
            let i = if sub == "--" { 2 } else { 1 };
            if a.get(i).is_some_and(|s| !s.starts_with('-')) {
                n.children.push(self.argv(&a[i..], cwd, depth + 1));
                n.confidence = Confidence::ResolvedNestedWrapper;
            }
            return n;
        }
        if ["npm", "pnpm", "yarn", "bun", "composer"].contains(&exe) {
            let mut dir = cwd.to_path_buf();
            let mut i = 1;
            let mut selector = None;
            let mut recursive = false;
            while i < a.len() {
                match a[i].as_str() {
                    "--filter" | "-F" | "--workspace" | "-w" => {
                        selector = a.get(i + 1).cloned();
                        i += 2;
                    }
                    "--dir" | "-C" | "--cwd" | "--prefix" | "--working-dir" => {
                        if let Some(p) = a.get(i + 1) {
                            dir = cwd.join(p);
                        }
                        i += 2;
                    }
                    "-r" | "--recursive" | "--workspaces" => {
                        recursive = true;
                        i += 1;
                    }
                    "--silent" | "--if-present" => i += 1,
                    _ => break,
                }
            }
            let Some(s) = a.get(i).map(String::as_str) else {
                return n;
            };
            if exe == "yarn" && s == "workspace" {
                selector = a.get(i + 1).cloned();
                i += 2;
            }
            let Some(s) = a.get(i).map(String::as_str) else {
                return n;
            };
            if ["exec", "dlx", "x"].contains(&s) {
                if a.get(i + 1).is_some_and(|v| !v.starts_with('-')) {
                    n.children.push(self.argv(&a[i + 1..], &dir, depth + 1));
                    n.confidence = Confidence::ResolvedNestedWrapper;
                }
                return n;
            }
            if exe == "bun" && s == "test" {
                n.family = Some("bun-test".into());
                n.confidence = Confidence::ExactDirectTool;
                return n;
            }
            if [
                "install",
                "add",
                "remove",
                "uninstall",
                "update",
                "upgrade",
                "publish",
                "ci",
                "audit",
                "outdated",
                "list",
                "ls",
            ]
            .contains(&s)
            {
                n.family = Some("packages".into());
                n.confidence = Confidence::ExactDirectTool;
                return n;
            }
            let script = if ["run", "run-script"].contains(&s) {
                a.get(i + 1).map(String::as_str).unwrap_or("")
            } else {
                s
            };
            if recursive || selector.is_some() {
                let dirs = workspaces(&dir);
                for p in dirs {
                    let name = json(&p.join("package.json"))
                        .and_then(|v| v["name"].as_str().map(str::to_owned))
                        .unwrap_or_default();
                    let selected = selector.as_ref().is_none_or(|s| {
                        s == &name
                            || glob::Pattern::new(s).is_ok_and(|g| {
                                g.matches(&name)
                                    || g.matches_path(p.strip_prefix(&dir).unwrap_or(&p))
                            })
                    });
                    if selected {
                        n.children.push(self.script(exe, script, &p, depth + 1));
                    }
                }
                if !n.children.is_empty() {
                    n.confidence = Confidence::ResolvedProjectScript;
                }
                return n;
            }
            let manifest = if exe == "composer" {
                "composer.json"
            } else {
                "package.json"
            };
            if project(&dir, manifest)
                .and_then(|p| json(&p.join(manifest)))
                .is_some_and(|v| !v["scripts"][script].is_null())
            {
                return self.script(exe, script, &dir, depth + 1);
            }
            if ["pnpm", "yarn", "bun"].contains(&exe) && !["run", "run-script"].contains(&s) {
                n.children.push(self.argv(&a[i..], &dir, depth + 1));
                n.confidence = Confidence::ResolvedNestedWrapper;
            }
            return n;
        }
        if ["turbo", "nx", "lerna"].contains(&exe) {
            let target = if sub == "run" {
                a.get(2).map(String::as_str)
            } else if exe == "nx" && sub == "run-many" {
                a.iter()
                    .position(|s| s == "-t" || s == "--target")
                    .and_then(|i| a.get(i + 1))
                    .map(String::as_str)
            } else {
                Some(sub)
            };
            if let Some(target) = target {
                let parts: Vec<_> = target.split(':').collect();
                let script = parts.last().copied().unwrap_or(target);
                for p in workspaces(cwd) {
                    let v = json(&p.join("package.json")).unwrap_or_default();
                    if parts.len() > 1
                        && v["name"].as_str() != Some(parts[0])
                        && p.file_name().and_then(|s| s.to_str()) != Some(parts[0])
                    {
                        continue;
                    }
                    if !v["scripts"][script].is_null() {
                        n.children.push(self.script("pnpm", script, &p, depth + 1));
                    }
                }
            }
            if !n.children.is_empty() {
                n.confidence = Confidence::ResolvedNestedWrapper;
            }
            return n;
        }
        if ["docker", "podman"].contains(&exe) && a.iter().any(|s| s == "exec" || s == "run") {
            n.notes
                .push("container filesystem not introspected; nested command is untrusted".into());
            if let Some(index) = a.iter().position(|s| s == "exec" || s == "run") {
                let mut i = index + 1;
                while i < a.len() && a[i].starts_with('-') {
                    if [
                        "-u",
                        "--user",
                        "-w",
                        "--workdir",
                        "-e",
                        "--env",
                        "--env-file",
                        "--entrypoint",
                    ]
                    .contains(&a[i].as_str())
                    {
                        i += 2;
                    } else if ["--rm", "-T", "-t", "-i", "-it", "--privileged"]
                        .contains(&a[i].as_str())
                    {
                        i += 1;
                    } else {
                        break;
                    }
                }
                if i < a.len() && !a[i].starts_with('-') && i + 1 < a.len() {
                    let nested = &a[i + 1..];
                    if nested.iter().any(|s| {
                        [
                            "npm", "pnpm", "yarn", "bun", "composer", "make", "just", "task", "nx",
                            "turbo",
                        ]
                        .contains(&s.as_str())
                    }) {
                        n.children.push(node(&nested.join(" "), cwd));
                    } else {
                        n.children.push(self.argv(nested, cwd, depth + 1));
                    }
                }
            }

            return n;
        }
        n.family = if exe == "git" && sub == "status" {
            Some("git-status".into())
        } else if ["rg", "ripgrep", "grep", "ag"].contains(&exe)
            && a.iter().any(|s| s == "-n" || s == "--line-number")
            && a.iter().any(|s| s == "-H" || s == "--with-filename")
            && !a.iter().any(|s| {
                [
                    "-h",
                    "--no-filename",
                    "--vimgrep",
                    "--heading",
                    "--only-matching",
                    "-o",
                    "--replace",
                    "-r",
                    "--column",
                    "--byte-offset",
                    "-b",
                    "--count",
                    "-c",
                    "--files-with-matches",
                    "-l",
                    "--files",
                    "--json",
                ]
                .contains(&s.as_str())
            })
        {
            Some("search-lines".into())
        } else {
            family(exe, sub).map(str::to_owned)
        };
        if n.family.is_some() {
            n.confidence = Confidence::ExactDirectTool;
        }
        n
    }
}
pub fn family(exe: &str, sub: &str) -> Option<&'static str> {
    Some(match exe {
        "cargo" => match sub {
            "test" | "nextest" => "cargo-test",
            "build" | "check" | "clippy" | "fmt" => "rust",
            _ => return None,
        },
        "rustc" => "rust",
        "go" => match sub {
            "test" => "go-test",
            "build" | "vet" => "go",
            _ => return None,
        },
        "golangci-lint" | "staticcheck" => "go",
        "vitest" | "jest" | "playwright" | "cypress" => "js-test",
        "node" if sub == "--test" => "js-test",
        "eslint" | "biome" | "oxlint" | "prettier" => "js-lint",
        "tsc" | "vue-tsc" => "typescript",
        "vite" | "next" | "nuxt" | "webpack" | "rollup" | "esbuild" | "tsup" => "js-build",
        "pytest" => "pytest",
        "ruff" | "mypy" | "pyright" | "basedpyright" | "ty" => "python-lint",
        "pip" | "pip3" | "pipx" => "packages",
        "tox" | "nox" => "python-task",
        "phpunit" | "pest" => "php-test",
        "phpstan" | "psalm" | "phpcs" | "php-cs-fixer" => "php-lint",
        "artisan" => "php-task",
        "mvn" | "maven" | "gradle" | "gradlew" => "jvm",
        "javac" => "compiler",
        "dotnet" => "dotnet",
        "rspec" => "ruby-test",
        "rubocop" => "ruby-lint",
        "rake" => "ruby-task",
        "cmake" | "ninja" | "make" | "meson" => "native-build",
        "gcc" | "g++" | "clang" | "clang++" | "clang-tidy" => "compiler",
        "docker" | "podman" => "container",
        "git" => "git",
        "gh" | "glab" | "gt" | "jj" => "forge",
        "rg" | "ripgrep" | "grep" | "ag" => "search",
        "ls" | "tree" | "find" | "fd" | "du" | "df" | "stat" => "filesystem",
        "kubectl" | "helm" | "terraform" | "tofu" | "aws" | "gcloud" | "az" | "ansible"
        | "ansible-playbook" | "pulumi" => "infrastructure",
        "curl" | "wget" => "network",
        "just" | "task" => "task",
        "swift" | "xcodebuild" | "sbt" | "scalac" | "mix" | "quarto" | "pio" => "other-build",
        "shellcheck" | "hadolint" | "markdownlint" | "yamllint" => "other-lint",
        _ => return None,
    })
}
fn graph_info(n: &WorkloadGraph, families: &mut BTreeSet<String>) -> bool {
    if let Some(f) = &n.family {
        families.insert(f.clone());
    }
    let mut unknown = n.confidence == Confidence::Unknown;
    for c in &n.children {
        unknown |= graph_info(c, families);
    }
    unknown
}
fn command_risk(a: &[String]) -> Risk {
    let Some(first) = a.first() else {
        return Risk::Unknown;
    };
    let exe = Path::new(first)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    if [
        "sudo", "su", "rm", "chmod", "chown", "dd", "mkfs", "reboot", "shutdown",
    ]
    .contains(&exe.as_ref())
    {
        return Risk::Destructive;
    }
    if [
        "vim", "vi", "nano", "less", "more", "top", "htop", "bash", "sh", "zsh", "fish", "psql",
        "mysql", "ssh",
    ]
    .contains(&exe.as_ref())
        || a.iter().any(|s| {
            [
                "--watch",
                "-w",
                "--interactive",
                "-it",
                "--tty",
                "dev",
                "serve",
                "watch",
            ]
            .contains(&s.as_str())
        })
    {
        return Risk::Interactive;
    }
    if a.iter().any(|s| {
        [
            "push", "reset", "clean", "checkout", "switch", "apply", "destroy", "delete", "remove",
            "install", "add", "publish", "upgrade", "update", "--fix", "--write", "-f",
        ]
        .contains(&s.as_str())
    }) {
        return Risk::Mutating;
    }
    if ["git", "rg", "grep", "ls", "tree", "find", "fd", "du"].contains(&exe.as_ref()) {
        Risk::ReadOnly
    } else {
        Risk::Diagnostic
    }
}
pub fn classify(source: &str, cwd: &Path, shell: Shell) -> Classification {
    let ast = parse(source, shell);
    let workload = Resolver::default().text(source, cwd, 0);
    let mut families = BTreeSet::new();
    let unknown = graph_info(&workload, &mut families);
    let mut risk = ast
        .commands
        .iter()
        .map(|c| command_risk(&c.argv))
        .max()
        .unwrap_or(Risk::Unknown);
    fn nested_risk(n: &WorkloadGraph) -> Risk {
        let a = shell_words::split(&n.label).unwrap_or_default();
        n.children
            .iter()
            .map(nested_risk)
            .fold(command_risk(&a), std::cmp::max)
    }
    risk = std::cmp::max(risk, nested_risk(&workload));
    let exact = ast.exact_output
        || ast.commands.iter().any(|c| {
            c.argv.iter().any(|a| {
                [
                    "--json",
                    "--porcelain",
                    "--raw",
                    "--no-filter",
                    "--format=json",
                    "--output=json",
                    "--output=yaml",
                    "--numstat",
                    "--name-only",
                    "--name-status",
                    "--oneline",
                    "-z",
                    "--null",
                    "--null-data",
                ]
                .contains(&a.as_str())
            }) || c.argv.windows(2).any(|a| {
                ["-o", "--output", "--format"].contains(&a[0].as_str())
                    && ["json", "yaml", "xml", "jsonl", "sarif"].contains(&a[1].as_str())
            })
        });
    let mut reasons = vec![];
    if unknown {
        reasons.push("unresolved workload; retain raw output".into());
    }
    if ast.uncertain {
        reasons.push("shell syntax requires passthrough".into());
    }
    if exact {
        reasons.push("exact or machine-consumed output".into());
    }
    let filterable =
        !unknown && !ast.uncertain && !exact && !families.is_empty() && risk != Risk::Interactive;
    Classification {
        schema_version: 1,
        ast,
        workload,
        risk,
        families: families.into_iter().collect(),
        filterable,
        hook_action: "passthrough".into(),
        reasons,
    }
}

/// Cross-invocation cache of resolved graphs, invalidated by content and membership.
/// Failure to read/write a cache never prevents fresh classification.
pub fn classify_cached(source: &str, cwd: &Path, shell: Shell, cache_dir: &Path) -> Classification {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Cached {
        version: u32,
        created: u64,
        stamp: String,
        class: Classification,
    }
    fn dirs(n: &WorkloadGraph, out: &mut BTreeSet<PathBuf>) {
        out.insert(n.cwd.clone());
        for c in &n.children {
            dirs(c, out);
        }
    }
    fn stamp(cwd: &Path, graph: &WorkloadGraph) -> Option<String> {
        let mut locations = BTreeSet::new();
        dirs(graph, &mut locations);
        locations.insert(cwd.into());
        for p in cwd.ancestors() {
            locations.insert(p.into());
            if p.join(".git").exists() {
                break;
            }
        }
        let initial: Vec<_> = locations.iter().cloned().collect();
        for p in initial {
            locations.extend(workspaces(&p));
            if locations.len() > 512 {
                return None;
            }
        }
        let mut hash = Sha256::new();
        let mut bytes_read = 0usize;
        for p in locations {
            hash.update(p.as_os_str().as_encoded_bytes());
            for name in [
                "package.json",
                "pnpm-workspace.yaml",
                ".npmrc",
                ".yarnrc.yml",
                "composer.json",
                "nx.json",
                "project.json",
                "turbo.json",
                "Makefile",
                "makefile",
                "GNUmakefile",
                "justfile",
                "Justfile",
                ".justfile",
                "Taskfile.yml",
                "Taskfile.yaml",
            ] {
                let path = p.join(name);
                hash.update(name.as_bytes());
                if path.exists() {
                    let meta = std::fs::metadata(&path).ok()?;
                    if meta.len() > 4 * 1024 * 1024 {
                        return None;
                    }
                    let bytes = std::fs::read(&path).ok()?;
                    bytes_read += bytes.len();
                    if bytes_read > 16 * 1024 * 1024 {
                        return None;
                    }
                    hash.update(bytes);
                } else {
                    hash.update([0]);
                }
            }
        }
        Some(format!("{:x}", hash.finalize()))
    }
    let key = format!(
        "{:x}",
        Sha256::digest(format!("{shell:?}\0{}\0{source}", cwd.display()).as_bytes())
    );
    let path = cache_dir.join(format!("{key}.json"));
    if let Ok(bytes) = std::fs::read(&path)
        && bytes.len() < 256 * 1024
        && let Ok(cached) = serde_json::from_slice::<Cached>(&bytes)
        && cached.version == 3
        && crate::recovery::now().saturating_sub(cached.created) < 86400
        && cached.class.ast.source == source
        && stamp(cwd, &cached.class.workload).as_deref() == Some(&cached.stamp)
    {
        return cached.class;
    }
    let class = classify(source, cwd, shell);
    if class.filterable
        && class.risk <= Risk::Diagnostic
        && let Some(stamp) = stamp(cwd, &class.workload)
        && let Ok(bytes) = serde_json::to_vec(&Cached {
            version: 3,
            created: crate::recovery::now(),
            stamp,
            class: class.clone(),
        })
        && bytes.len() < 256 * 1024
    {
        let _ = crate::recovery::atomic_write(&path, &bytes);
        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            let mut entries: Vec<_> = entries
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|s| s == "json"))
                .collect();
            entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
            let extra = entries.len().saturating_sub(32);
            for e in entries.into_iter().take(extra) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    class
}
