//! Conservative, read-only hints for output filtering.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::manifests::WorkspaceHints;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Test,
    Lint,
    Typecheck,
    Build,
    Format,
    Install,
    RustTest,
    RustNextest,
    RustBuild,
    RustCheck,
    RustClippy,
    RustFmt,
    RustDoc,
    PyTest,
    PyUnittest,
    PyTox,
    PyNox,
    PyRuff,
    PyMypy,
    PyPyright,
    PyPylint,
    PyBlack,
    PyCoverage,
    PyInstall,
    GoTest,
    GoJson,
    GoBuild,
    GoVet,
    GoGenerate,
    GoLint,
    GoStaticcheck,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ManifestHints {
    pub workspace: WorkspaceHints,
}

impl ManifestHints {
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn load(directory: &Path) -> Option<Self> {
        Self::discover(directory).ok().flatten()
    }

    pub fn discover(directory: &Path) -> Result<Option<Self>, super::manifests::DiscoveryError> {
        Ok(WorkspaceHints::discover(directory)?.map(|workspace| Self { workspace }))
    }
}

#[derive(Default)]
struct LocalDiscovery {
    attempted: bool,
    hints: Option<Rc<ManifestHints>>,
}

impl LocalDiscovery {
    fn discover(&mut self, directory: &Path) -> Result<Rc<ManifestHints>, ()> {
        if !self.attempted {
            self.attempted = true;
            self.hints = ManifestHints::discover(directory)
                .map_err(|_| ())?
                .map(Rc::new);
        }
        self.hints
            .as_ref()
            .filter(|hints| hints.workspace.project_at(directory).is_some())
            .cloned()
            .ok_or(())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Plan {
    pub families: Vec<Family>,
    pub raw: bool,
    pub fallback: bool,
    pub sources: Vec<String>,
    pub source_aliases: Vec<(String, String)>,
    pub fallback_only_prefixed: bool,
}

impl Plan {
    fn add(&mut self, family: Family) {
        if !self.families.contains(&family) {
            self.families.push(family);
        }
    }
}

pub(crate) fn classify(arguments: &[OsString], cwd: &Path, hints: Option<&ManifestHints>) -> Plan {
    let shell_mode = arguments.len() == 1;
    let words = if shell_mode {
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
    if is_go_json_command(&words) {
        plan.add(Family::GoJson);
        return plan;
    }
    if machine_output(&words) {
        plan.raw = true;
        return plan;
    }
    let mut directory = cwd.to_path_buf();
    let mut local_discovery = LocalDiscovery::default();
    let root = hints.map_or_else(
        || cwd.to_path_buf(),
        |manifest| manifest.workspace.root.clone(),
    );
    if !shell_mode {
        classify_segment(
            &words,
            &mut directory,
            &root,
            hints,
            &mut plan,
            1,
            &mut Vec::new(),
            &mut local_discovery,
        );
        return plan;
    }
    let mut segment: Vec<String> = Vec::new();
    for word in words.into_iter().chain(std::iter::once(";".to_owned())) {
        if word == "&&" || word == ";" {
            let restored: Vec<String> = segment
                .iter()
                .map(|word: &String| restore_literal(word))
                .collect();
            classify_segment(
                &restored,
                &mut directory,
                &root,
                hints,
                &mut plan,
                1,
                &mut Vec::new(),
                &mut local_discovery,
            );
            segment.clear();
        } else if matches!(
            word.as_str(),
            "|" | "||" | ">" | "<" | "&" | ">>" | "<=" | "2>"
        ) {
            return Plan {
                raw: true,
                ..Plan::default()
            };
        } else {
            segment.push(word);
        }
    }
    plan
}

pub(crate) fn needs_manifest_discovery(arguments: &[OsString]) -> bool {
    let words = if arguments.len() == 1 {
        arguments[0]
            .to_str()
            .and_then(split_shell)
            .unwrap_or_default()
    } else {
        arguments
            .iter()
            .filter_map(|arg| arg.to_str().map(str::to_owned))
            .collect()
    };
    let mut segment: Vec<String> = Vec::new();
    for word in words.iter().chain(std::iter::once(&";".to_owned())) {
        if matches!(word.as_str(), "&&" | ";") {
            let mut command = segment.as_slice();
            while command.first().is_some_and(|word| is_assignment(word)) {
                command = &command[1..];
            }
            if command.first().is_some_and(|word| word == "env") {
                command = &command[1..];
                while command.first().is_some_and(|word| is_assignment(word)) {
                    command = &command[1..];
                }
            }
            if let Some(program) = command.first() {
                let name = Path::new(program)
                    .file_name()
                    .and_then(|x| x.to_str())
                    .unwrap_or(program);
                if matches!(
                    name,
                    "npm"
                        | "pnpm"
                        | "yarn"
                        | "bun"
                        | "npx"
                        | "pnpx"
                        | "bunx"
                        | "turbo"
                        | "nx"
                        | "lerna"
                        | "lage"
                        | "moon"
                        | "cargo"
                        | "go"
                ) {
                    return true;
                }
            }
            segment.clear();
        } else {
            segment.push(word.clone());
        }
    }
    false
}

pub(crate) fn has_background_shell_job(arguments: &[OsString]) -> bool {
    let command = if arguments.len() == 1 {
        arguments[0].to_str()
    } else {
        let shell = arguments
            .first()
            .and_then(|program| program.to_str())
            .and_then(|program| Path::new(program).file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| matches!(name, "sh" | "bash" | "dash" | "zsh" | "ksh"));
        if !shell {
            return false;
        }
        let command_index = arguments.iter().position(|argument| {
            argument.to_str().is_some_and(|value| {
                value == "-c" || value.starts_with('-') && value[1..].contains('c')
            })
        });
        command_index
            .and_then(|index| arguments.get(index + 1))
            .and_then(|command| command.to_str())
    };
    command.is_some_and(|command| {
        let bytes = command.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'&' {
                if bytes.get(index + 1) == Some(&b'&') {
                    index += 2;
                    continue;
                }
                // Quoted ampersands may become shell operators in a nested
                // `sh -c` invocation. Treat them as background syntax and
                // preserve the native shell's inherited stdio behavior.
                return true;
            }
            index += 1;
        }
        false
    })
}

fn machine_output(words: &[String]) -> bool {
    words
        .iter()
        .any(|word| machine_flag(word) || word == "-json" || word.starts_with("-json="))
        || words.windows(2).any(|pair| {
            matches!(
                pair[0].as_str(),
                "--reporter" | "--format" | "--message-format"
            ) && matches!(
                pair[1].as_str(),
                "json" | "jsonl" | "json-render-diagnostics" | "xml" | "yaml" | "sarif" | "tap"
            )
        })
}

fn is_go_json_command(words: &[String]) -> bool {
    if words
        .iter()
        .any(|word| matches!(word.as_str(), "&&" | ";" | "|" | "||" | ">" | "<" | "&"))
    {
        return false;
    }
    let Some(program) = words.first() else {
        return false;
    };
    if Path::new(program).file_name().and_then(|x| x.to_str()) != Some("go") {
        return false;
    }
    let mut args = &words[1..];
    if args.first().is_some_and(|word| word == "-C") {
        if args.len() < 2 {
            return false;
        }
        args = &args[2..];
    }
    let flags = args.split(|word| word == "--").next().unwrap_or(args);
    flags.first().is_some_and(|word| word == "test")
        && flags
            .iter()
            .any(|word| word == "-json" || word == "-json=true")
        && !flags.iter().any(|word| word == "-json=false")
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
        "--message-format=json",
        "--message-format=json-render-diagnostics",
        "--reporter=json",
        "--reporter=xml",
        "--reporter=tap",
    ]
    .iter()
    .any(|prefix| word.starts_with(prefix))
}

#[allow(clippy::too_many_arguments)]
fn classify_segment(
    words: &[String],
    directory: &mut PathBuf,
    root: &Path,
    hints: Option<&ManifestHints>,
    plan: &mut Plan,
    depth: usize,
    seen: &mut Vec<String>,
    local_discovery: &mut LocalDiscovery,
) {
    if words.is_empty() {
        return;
    }
    let mut words = words;
    while words.first().is_some_and(|word| is_assignment(word)) {
        words = &words[1..];
    }
    let Some(program) = words.first() else { return };
    if program == "cd" {
        let Some(path) = words.get(1).filter(|_| words.len() == 2) else {
            plan.raw = true;
            return;
        };
        if path.starts_with('-') || path == "~" || path.contains(['*', '?', '[', ']']) {
            plan.raw = true;
            return;
        }
        let Ok(target) = std::fs::canonicalize(directory.join(path)) else {
            plan.raw = true;
            return;
        };
        if !target.is_dir() {
            plan.raw = true;
            return;
        }
        *directory = target;
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
        while let Some(option) = args.first() {
            if option == "--" {
                args = &args[1..];
                break;
            }
            if matches!(
                option.as_str(),
                "-p" | "--package" | "-w" | "--workspace" | "--prefix" | "--cache"
            ) {
                if args.get(1).is_none_or(|value| value.starts_with('-')) {
                    plan.raw = true;
                    return;
                }
                args = &args[2..];
            } else if ["--package=", "--workspace=", "--prefix=", "--cache="]
                .iter()
                .any(|prefix| option.starts_with(prefix) && option.len() > prefix.len())
                || matches!(
                    option.as_str(),
                    "-y" | "--yes"
                        | "-q"
                        | "--quiet"
                        | "--no-install"
                        | "--prefer-offline"
                        | "--offline"
                )
            {
                args = &args[1..];
            } else if option.starts_with('-') {
                plan.raw = true;
                return;
            } else {
                break;
            }
        }
        if depth > 16 {
            plan.raw = true;
        } else if !args.is_empty() {
            classify_segment(
                args,
                directory,
                root,
                hints,
                plan,
                depth + 1,
                seen,
                local_discovery,
            );
        } else {
            plan.raw = true;
        }
        return;
    }
    if matches!(name, "npm" | "pnpm" | "yarn" | "bun") {
        let Some(invocation) = parse_package_manager(name, args) else {
            plan.raw = true;
            return;
        };
        if invocation.dev_or_watch {
            plan.raw = true;
            return;
        }
        if invocation.install {
            plan.add(Family::Install);
            return;
        }
        if invocation.bun_test {
            plan.add(Family::Test);
            return;
        }
        if invocation.unsupported {
            plan.raw = true;
            return;
        }
        let Some(script) = invocation.script.as_deref() else {
            return;
        };
        let effective_directory = if let Some(prefix) = invocation.prefix.as_deref() {
            let prefix = Path::new(prefix);
            let prefix = if prefix.is_absolute() {
                prefix.to_path_buf()
            } else {
                directory.join(prefix)
            };
            match std::fs::canonicalize(prefix) {
                Ok(path) if path.is_dir() => path,
                _ => {
                    plan.raw = true;
                    return;
                }
            }
        } else {
            directory.to_path_buf()
        };
        let local_hints =
            if hints.is_none_or(|h| h.workspace.project_at(&effective_directory).is_none()) {
                match local_discovery.discover(&effective_directory) {
                    Ok(hints) => Some(hints),
                    Err(()) => {
                        plan.raw = true;
                        return;
                    }
                }
            } else {
                None
            };
        let selected_hints = hints
            .filter(|h| h.workspace.project_at(&effective_directory).is_some())
            .or(local_hints.as_deref());
        let Some(selected_hints) = selected_hints else {
            plan.raw = true;
            return;
        };
        let projects = if invocation.workspace {
            let static_selectors: Vec<String> = invocation
                .selectors
                .iter()
                .filter(|selector| !selector.contains('['))
                .cloned()
                .collect();
            let mut selected = selected_hints.workspace.select_packages(&static_selectors);
            if !invocation.include_root {
                selected.retain(|project| project.path != selected_hints.workspace.root);
            }
            if !invocation.selectors.is_empty() && selected.is_empty() {
                plan.raw = true;
                return;
            }
            selected
        } else {
            selected_hints
                .workspace
                .project_at(&effective_directory)
                .into_iter()
                .collect()
        };
        if invocation.workspace && projects.len() > 1 {
            // Several package managers print package scripts without a source
            // envelope. Keep those records until the runner identifies the
            // package in its own prefix.
            plan.fallback_only_prefixed = true;
        }
        if invocation.dynamic_selection {
            plan.fallback = true;
        }
        let mut resolved = false;
        let run_lifecycle = match name {
            "npm" | "bun" => true,
            "pnpm" => selected_hints.workspace.pnpm_pre_post == Some(true),
            "yarn" => selected_hints.workspace.yarn_classic == Some(true),
            _ => false,
        } && !invocation.ignore_scripts;
        for project in projects {
            let body = project.scripts.get(script).map(String::as_str);
            let Some(body) = body else {
                continue;
            };
            resolved = true;
            push_project_source(plan, &selected_hints.workspace, project);
            if run_lifecycle {
                for lifecycle in [format!("pre{script}"), format!("post{script}")] {
                    if let Some(lifecycle_body) = project.scripts.get(&lifecycle) {
                        resolve_script(
                            name,
                            &project.path,
                            &lifecycle,
                            lifecycle_body,
                            selected_hints,
                            &selected_hints.workspace.root,
                            plan,
                            depth,
                            seen,
                            local_discovery,
                        );
                    }
                }
            } else if name == "yarn"
                && selected_hints.workspace.yarn_classic.is_none()
                && [format!("pre{script}"), format!("post{script}")]
                    .iter()
                    .any(|lifecycle| project.scripts.contains_key(lifecycle))
            {
                plan.raw = true;
            }
            resolve_script(
                name,
                &project.path,
                script,
                body,
                selected_hints,
                &selected_hints.workspace.root,
                plan,
                depth,
                seen,
                local_discovery,
            );
        }
        if !resolved {
            plan.raw = true;
        }
        return;
    }
    if classify_monorepo_runner(
        name,
        args,
        directory,
        root,
        hints,
        plan,
        depth,
        seen,
        local_discovery,
    ) {
        return;
    }
    classify_tool(name, args, plan);
}

#[allow(clippy::too_many_arguments)]
fn resolve_script(
    manager: &str,
    directory: &Path,
    script: &str,
    body: &str,
    hints: &ManifestHints,
    root: &Path,
    plan: &mut Plan,
    depth: usize,
    seen: &mut Vec<String>,
    local_discovery: &mut LocalDiscovery,
) {
    let id = format!("{manager}:{}:{script}", directory.display());
    if seen.contains(&id) {
        plan.raw = true;
        return;
    }
    if depth > 16 {
        plan.raw = true;
        return;
    }
    let Some(body_words) = split_shell(body) else {
        plan.raw = true;
        return;
    };
    if machine_output(&body_words) {
        plan.raw = true;
        return;
    }
    seen.push(id);
    let mut segment = Vec::new();
    let mut current = directory.to_path_buf();
    for word in body_words
        .into_iter()
        .chain(std::iter::once(";".to_owned()))
    {
        if matches!(word.as_str(), "&&" | ";") {
            let restored: Vec<String> = segment
                .iter()
                .map(|word: &String| restore_literal(word))
                .collect();
            let mut resolved_plan = Plan::default();
            classify_segment(
                &restored,
                &mut current,
                root,
                Some(hints),
                &mut resolved_plan,
                depth + 1,
                seen,
                local_discovery,
            );
            let is_cd = restored.first().is_some_and(|word| word == "cd");
            if resolved_plan.families.is_empty() && !resolved_plan.raw && !is_cd {
                resolved_plan.raw = true;
            }
            plan.raw |= resolved_plan.raw;
            plan.fallback |= resolved_plan.fallback;
            plan.fallback_only_prefixed |= resolved_plan.fallback_only_prefixed;
            for family in resolved_plan.families {
                plan.add(family);
            }
            for source in resolved_plan.sources {
                push_source(plan, &source);
            }
            for (alias, source) in resolved_plan.source_aliases {
                if !plan.source_aliases.iter().any(|entry| entry.0 == alias) {
                    plan.source_aliases.push((alias, source));
                }
            }
            segment.clear();
        } else if matches!(word.as_str(), "|" | "||" | ">" | "<" | "&" | ">>" | "2>") {
            plan.raw = true;
            break;
        } else {
            segment.push(word);
        }
    }
    seen.pop();
}

#[derive(Default)]
struct PackageManagerInvocation {
    script: Option<String>,
    selectors: Vec<String>,
    workspace: bool,
    include_root: bool,
    dynamic_selection: bool,
    unsupported: bool,
    install: bool,
    bun_test: bool,
    dev_or_watch: bool,
    ignore_scripts: bool,
    prefix: Option<String>,
}

fn parse_package_manager(manager: &str, args: &[String]) -> Option<PackageManagerInvocation> {
    let mut result = PackageManagerInvocation::default();
    let mut command = Vec::new();
    let mut index = 0;
    let mut forwarded = false;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--" {
            forwarded = true;
            command.push((index, arg.clone()));
            index += 1;
            continue;
        }
        if forwarded {
            if matches!(arg.as_str(), "dev" | "watch") {
                result.dev_or_watch = true;
            }
            command.push((index, arg.clone()));
            index += 1;
            continue;
        }
        if manager == "npm"
            && arg.starts_with("-w")
            && !arg.starts_with("--")
            && !arg.starts_with("-w=")
            && arg.len() > 2
        {
            result.workspace = true;
            result.selectors.push(arg[2..].to_owned());
            index += 1;
            continue;
        }
        if matches!(
            arg.as_str(),
            "--workspace"
                | "-w"
                | "--filter"
                | "-F"
                | "--dir"
                | "-C"
                | "--prefix"
                | "--from"
                | "--since"
        ) {
            let value = args.get(index + 1)?;
            match arg.as_str() {
                "--prefix" if manager == "npm" => {
                    result.prefix = Some(value.clone());
                }
                "--workspace" | "-w" | "--filter" | "-F" => {
                    result.workspace = true;
                    result.selectors.push(value.clone());
                    if value.contains('[') {
                        result.dynamic_selection = true;
                    }
                }
                "--dir" | "-C" => {
                    result.workspace = true;
                    result.selectors.push(value.clone());
                }
                "--since" => {
                    result.workspace = true;
                    result.dynamic_selection = true;
                }
                "--from" => {
                    result.workspace = true;
                    result.selectors.push(value.clone());
                }
                _ => {}
            }
            index += 2;
            continue;
        }
        if let Some((flag, value)) = arg.split_once('=')
            && (matches!(
                flag,
                "--workspace" | "-w" | "--filter" | "-F" | "--dir" | "--from" | "--since"
            ) || flag == "--prefix" && manager == "npm")
        {
            if flag == "--prefix" {
                result.prefix = Some(value.to_owned());
            } else if flag == "--since" {
                result.workspace = true;
                result.dynamic_selection = true;
            } else {
                result.workspace = true;
                result.selectors.push(value.to_owned());
            }
            index += 1;
            continue;
        }
        if matches!(
            arg.as_str(),
            "--workspaces" | "-r" | "--recursive" | "-A" | "--all" | "--include-workspace-root"
        ) {
            result.workspace = true;
            if arg == "--include-workspace-root"
                || manager == "yarn" && matches!(arg.as_str(), "-A" | "--all")
            {
                result.include_root = true;
            }
            index += 1;
            continue;
        }
        if arg == "--ignore-scripts" {
            result.ignore_scripts = true;
            index += 1;
            continue;
        }
        if arg.starts_with('-') {
            if matches!(
                arg.as_str(),
                "--loglevel"
                    | "--reporter"
                    | "--prefix"
                    | "--concurrency"
                    | "--jobs"
                    | "--include"
                    | "--exclude"
                    | "--scope"
                    | "--ignore"
            ) {
                if args
                    .get(index + 1)
                    .is_none_or(|value| value.starts_with('-'))
                {
                    result.unsupported = true;
                    index += 1;
                    continue;
                }
                if arg == "--prefix" && manager == "npm" {
                    result.prefix = args.get(index + 1).cloned();
                }
                index += 2;
                continue;
            }
            if matches!(
                arg.as_str(),
                "--silent"
                    | "--quiet"
                    | "--verbose"
                    | "--no-color"
                    | "--color"
                    | "--if-present"
                    | "--no-audit"
                    | "--no-fund"
                    | "--prefer-offline"
                    | "--offline"
                    | "--immutable"
                    | "--frozen-lockfile"
                    | "--no-optional"
                    | "--no-save"
                    | "--production"
                    | "--dev"
                    | "--force"
                    | "--parallel"
                    | "--topological"
                    | "--topological-dev"
                    | "--continue"
                    | "--no-private"
                    | "--workspace-root"
                    | "--no-workspace-root"
            ) {
                index += 1;
                continue;
            }
            result.unsupported = true;
            command.push((index, arg.clone()));
            index += 1;
            continue;
        }
        if matches!(arg.as_str(), "dev" | "watch") {
            result.dev_or_watch = true;
        }
        if matches!(arg.as_str(), "workspace" | "foreach") && manager == "yarn" {
            result.workspace = true;
        }
        command.push((index, arg.clone()));
        index += 1;
    }
    if command
        .first()
        .is_some_and(|(_, value)| matches!(value.as_str(), "ci" | "install"))
        && !result.workspace
    {
        result.install = true;
    }
    if manager == "bun"
        && command.iter().any(|(_, value)| value == "test")
        && !command.iter().any(|(_, value)| value == "run")
    {
        result.bun_test = true;
        return Some(result);
    }
    if manager == "yarn" && args.first().is_some_and(|v| v == "workspace") {
        result.workspace = true;
        let package = args.get(1)?;
        result.selectors.push(package.clone());
        result.script = args.iter().skip(2).find(|v| v.as_str() != "run").cloned();
    } else if manager == "yarn"
        && args.first().is_some_and(|v| v == "workspaces")
        && args.get(1).is_some_and(|v| v == "run")
    {
        result.workspace = true;
        result.script = args.get(2).cloned();
    } else if manager == "yarn" && args.iter().any(|v| v == "foreach") {
        result.workspace = true;
        let run_index = args
            .iter()
            .position(|v| v == "run")
            .or_else(|| args.iter().position(|v| v == "exec"))?;
        result.script = args.get(run_index + 1).cloned();
        if args.iter().any(|v| matches!(v.as_str(), "-A" | "--all")) {
            result.include_root = true;
        }
    } else {
        let values: Vec<&String> = command.iter().map(|(_, value)| value).collect();
        if let Some(run) = values.iter().position(|v| v.as_str() == "run") {
            result.script = values.get(run + 1).map(|v| (*v).clone());
        } else if let Some(candidate) = values.iter().find(|v| {
            matches!(
                v.as_str(),
                "test" | "lint" | "typecheck" | "build" | "check"
            )
        }) {
            result.script = Some((**candidate).clone());
        } else if result.workspace {
            result.unsupported = true;
        }
    }
    Some(result)
}

#[allow(clippy::too_many_arguments)]
fn classify_monorepo_runner(
    name: &str,
    args: &[String],
    directory: &Path,
    root: &Path,
    hints: Option<&ManifestHints>,
    plan: &mut Plan,
    depth: usize,
    seen: &mut Vec<String>,
    local_discovery: &mut LocalDiscovery,
) -> bool {
    let (task, workspace) = match name {
        "turbo" if args.first().is_some_and(|a| a == "run") => {
            (args.get(1).map(String::as_str), true)
        }
        "nx" if args.first().is_some_and(|a| a == "run") => {
            (args.get(1).and_then(|v| v.split(':').nth(1)), true)
        }
        "nx" if args.first().is_some_and(|a| task_family(a).is_some()) => {
            (args.first().map(String::as_str), true)
        }
        "nx" if args
            .first()
            .is_some_and(|a| matches!(a.as_str(), "run-many" | "affected")) =>
        {
            let target = args
                .windows(2)
                .find(|pair| matches!(pair[0].as_str(), "-t" | "--target" | "--targets"))
                .map(|pair| pair[1].as_str())
                .or_else(|| args.iter().find_map(|a| a.strip_prefix("--target=")));
            (target, true)
        }
        "lerna" if args.first().is_some_and(|a| a == "run") => {
            (args.get(1).map(String::as_str), true)
        }
        "lage" => (args.first().map(String::as_str), true),
        "moon"
            if args
                .first()
                .is_some_and(|a| matches!(a.as_str(), "run" | "r")) =>
        {
            (args.get(1).map(String::as_str), true)
        }
        _ => return false,
    };
    let Some(mut task) = task else {
        plan.raw = true;
        return true;
    };
    if name == "moon" && args.get(1).is_some_and(|target| target.starts_with("~:")) {
        plan.raw = true;
        return true;
    }
    if task.starts_with(':') {
        task = &task[1..];
    }
    if task.contains(':') {
        task = task.rsplit(':').next().unwrap_or(task);
    }
    if task.contains(',') {
        plan.raw = true;
        return true;
    }
    if !runner_options_supported(name, args) {
        plan.raw = true;
        return true;
    }
    let local_hints = if hints.is_none() {
        match local_discovery.discover(directory) {
            Ok(hints) => Some(hints),
            Err(()) => {
                plan.raw = true;
                return true;
            }
        }
    } else {
        None
    };
    let Some(hints) = hints.or(local_hints.as_deref()) else {
        plan.raw = true;
        return true;
    };
    let (includes, excludes, dynamic_selection) = match runner_selectors(name, args) {
        Ok(selectors) => selectors,
        Err(()) => {
            plan.raw = true;
            return true;
        }
    };
    if dynamic_selection {
        plan.fallback = true;
        plan.fallback_only_prefixed = true;
    }
    let mut projects = hints
        .workspace
        .projects
        .iter()
        .filter(|project| {
            if name == "moon" {
                project.moon_tasks.contains_key(task)
            } else if name == "nx" {
                project.nx_targets.contains_key(task) || project.scripts.contains_key(task)
            } else {
                project.scripts.contains_key(task)
            }
        })
        .collect::<Vec<_>>();
    if !includes.is_empty() || !excludes.is_empty() || dynamic_selection {
        let selected =
            runner_project_selection(&hints.workspace, &includes, &excludes, dynamic_selection);
        projects.retain(|project| selected.contains(&project.path));
    } else {
        projects.retain(|project| project.path != hints.workspace.root);
    }
    if (!includes.is_empty() || !excludes.is_empty()) && projects.is_empty() {
        plan.raw = true;
        return true;
    }
    if name == "nx"
        && args.first().is_some_and(|arg| arg == "run")
        && let Some((project_name, _)) = args.get(1).and_then(|target| target.split_once(':'))
    {
        projects.retain(|project| project.name.as_deref() == Some(project_name));
    }
    if name == "nx"
        && args.first().is_some_and(|arg| task_family(arg).is_some())
        && let Some(project_name) = args.get(1)
        && !project_name.starts_with('-')
    {
        projects.retain(|project| {
            project.name.as_deref() == Some(project_name)
                || project.path.file_name().and_then(|name| name.to_str())
                    == Some(project_name.as_str())
        });
    }
    if name == "moon"
        && let Some((project_name, _)) = args.get(1).and_then(|target| target.split_once(':'))
        && !project_name.is_empty()
    {
        projects.retain(|project| project.name.as_deref() == Some(project_name));
    }
    if projects.is_empty() {
        let declared = match name {
            "turbo" => hints.workspace.turbo_tasks.contains(task),
            "nx" => hints.workspace.nx_targets.contains(task),
            "lerna" => !hints.workspace.lerna_tasks.is_empty(),
            "moon" => hints.workspace.moon_tasks.contains(task),
            _ => false,
        };
        if declared && let Some(family) = task_family(task) {
            for project in hints
                .workspace
                .projects
                .iter()
                .filter(|project| project.path != hints.workspace.root)
            {
                push_project_source(plan, &hints.workspace, project);
            }
            if plan.sources.is_empty() {
                plan.raw = true;
            } else {
                plan.add(family);
                plan.fallback = true;
                plan.fallback_only_prefixed = true;
            }
        } else {
            plan.raw = true;
        }
        return true;
    }
    if projects.len() > 1 {
        plan.fallback_only_prefixed = true;
    }
    for project in projects {
        push_project_source(plan, &hints.workspace, project);
        let body = if name == "moon" {
            project.moon_tasks.get(task)
        } else if name == "nx" {
            project
                .nx_targets
                .get(task)
                .or_else(|| project.scripts.get(task))
        } else {
            project.scripts.get(task)
        };
        if let Some(body) = body {
            resolve_script(
                name,
                &project.path,
                task,
                body,
                hints,
                root,
                plan,
                depth,
                seen,
                local_discovery,
            );
        }
    }
    let _ = (directory, workspace);
    true
}

fn runner_options_supported(name: &str, args: &[String]) -> bool {
    let positional = match name {
        "turbo" | "lerna" | "moon" => 2,
        "lage" => 1,
        "nx" if args.first().is_some_and(|value| value == "run") => 2,
        "nx" if args
            .first()
            .is_some_and(|value| task_family(value).is_some()) =>
        {
            2
        }
        "nx" => 1,
        _ => return false,
    };
    let mut index = positional;
    while index < args.len() {
        let flag = args[index].as_str();
        let value_flag = match name {
            "turbo" => matches!(flag, "--filter" | "--concurrency" | "--cache-dir"),
            "nx" => matches!(
                flag,
                "-t" | "--target"
                    | "--targets"
                    | "--projects"
                    | "-p"
                    | "--exclude"
                    | "--output-style"
                    | "--parallel"
            ),
            "lerna" => matches!(flag, "--scope" | "--ignore" | "--since" | "--concurrency"),
            "lage" => matches!(flag, "--scope" | "--concurrency"),
            "moon" => matches!(flag, "--platform" | "--affected"),
            _ => false,
        };
        if value_flag {
            if args
                .get(index + 1)
                .is_none_or(|value| value.starts_with('-'))
            {
                return false;
            }
            index += 2;
            continue;
        }
        let (option, value) = flag.split_once('=').unwrap_or((flag, ""));
        if !value.is_empty()
            && match name {
                "turbo" => matches!(option, "--filter" | "--concurrency" | "--cache-dir"),
                "nx" => matches!(
                    option,
                    "--target"
                        | "--targets"
                        | "--projects"
                        | "--exclude"
                        | "--output-style"
                        | "--parallel"
                ),
                "lerna" => matches!(option, "--scope" | "--ignore" | "--since" | "--concurrency"),
                "lage" => matches!(option, "--scope" | "--concurrency"),
                "moon" => matches!(option, "--platform" | "--affected"),
                _ => false,
            }
        {
            index += 1;
            continue;
        }
        if matches!(
            flag,
            "--verbose"
                | "--no-cache"
                | "--skip-nx-cache"
                | "--parallel"
                | "--stream"
                | "--all"
                | "--topological"
                | "--continue"
                | "--affected"
        ) {
            index += 1;
            continue;
        }
        return false;
    }
    true
}

fn runner_selectors(name: &str, args: &[String]) -> Result<(Vec<String>, Vec<String>, bool), ()> {
    let (include_options, exclude_options): (&[&str], &[&str]) = match name {
        "turbo" => (&["--filter"], &[]),
        "nx" => (&["--projects", "-p"], &["--exclude"]),
        "lerna" => (&["--scope"], &["--ignore"]),
        "lage" => (&["--scope"], &[]),
        _ => (&[], &[]),
    };
    let includes = option_values(args, include_options)?;
    let excludes = option_values(args, exclude_options)?;
    let dynamic = name == "nx" && args.first().is_some_and(|arg| arg == "affected")
        || name == "lerna" && args.iter().any(|arg| arg == "--since")
        || includes
            .iter()
            .chain(&excludes)
            .any(|selector| selector.contains(['[', ']']) || selector.contains("..."));
    let includes = includes
        .into_iter()
        .flat_map(|selector| selector.split(',').map(str::to_owned).collect::<Vec<_>>())
        .collect();
    let excludes = excludes
        .into_iter()
        .flat_map(|selector| selector.split(',').map(str::to_owned).collect::<Vec<_>>())
        .collect();
    Ok((includes, excludes, dynamic))
}

fn option_values(args: &[String], options: &[&str]) -> Result<Vec<String>, ()> {
    let mut values = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if options.iter().any(|option| *option == argument) {
            let value = args.get(index + 1).filter(|value| !value.starts_with('-'));
            values.push(value.ok_or(())?.clone());
            index += 2;
        } else if let Some(value) = options.iter().find_map(|option| {
            argument
                .strip_prefix(option)
                .and_then(|rest| rest.strip_prefix('='))
                .filter(|value| !value.is_empty())
        }) {
            values.push(value.to_owned());
            index += 1;
        } else if options
            .iter()
            .any(|option| argument == &format!("{option}="))
        {
            return Err(());
        } else {
            index += 1;
        }
    }
    Ok(values)
}

fn runner_project_selection(
    workspace: &super::manifests::WorkspaceHints,
    includes: &[String],
    excludes: &[String],
    dynamic: bool,
) -> HashSet<PathBuf> {
    let root_name = workspace
        .projects
        .iter()
        .find(|project| project.path == workspace.root)
        .and_then(|project| project.name.as_deref());
    let include_root = includes.iter().any(|selector| {
        selector == "//" || selector == "." || root_name.is_some_and(|name| selector == name)
    });
    let includes = if dynamic {
        Vec::new()
    } else {
        includes
            .iter()
            .map(|selector| {
                if selector == "//" {
                    ".".to_owned()
                } else {
                    selector.clone()
                }
            })
            .collect::<Vec<_>>()
    };
    let selected = workspace.select_packages(&includes);
    let excluded: HashSet<PathBuf> = if dynamic || excludes.is_empty() {
        HashSet::new()
    } else {
        workspace
            .select_packages(excludes)
            .into_iter()
            .map(|project| project.path.clone())
            .collect()
    };
    let include_paths: HashSet<PathBuf> = selected
        .into_iter()
        .map(|project| project.path.clone())
        .collect();
    workspace
        .projects
        .iter()
        .filter(|project| {
            (dynamic || includes.is_empty() || include_paths.contains(&project.path))
                && (project.path != workspace.root || include_root)
                && !excluded.contains(&project.path)
        })
        .map(|project| project.path.clone())
        .collect()
}

fn push_source(plan: &mut Plan, source: &str) {
    if plan.sources.iter().any(|existing| existing == source) {
        return;
    }
    if plan.sources.len() >= 4096 {
        plan.raw = true;
        return;
    }
    plan.sources.push(source.to_owned());
}

fn push_project_source(
    plan: &mut Plan,
    workspace: &super::manifests::WorkspaceHints,
    project: &super::manifests::Project,
) {
    let path = project
        .path
        .strip_prefix(&workspace.root)
        .unwrap_or(Path::new("."));
    let path = path.to_string_lossy().replace('\\', "/");
    let identity = project.name.as_deref().unwrap_or(&path).to_owned();
    push_source(plan, &identity);
    for alias in std::iter::once(project.name.as_deref())
        .chain(project.aliases.iter().map(|alias| Some(alias.as_str())))
        .chain(std::iter::once(Some(path.as_str())))
        .flatten()
    {
        if let Some((_, existing_source)) = plan
            .source_aliases
            .iter()
            .find(|(existing_alias, _)| existing_alias == alias)
        {
            if existing_source != &identity {
                plan.raw = true;
            }
        } else {
            plan.source_aliases
                .push((alias.to_owned(), identity.clone()));
        }
    }
}

fn task_family(task: &str) -> Option<Family> {
    match task {
        "test" | "e2e" => Some(Family::Test),
        "lint" => Some(Family::Lint),
        "typecheck" | "type-check" => Some(Family::Typecheck),
        "build" => Some(Family::Build),
        "format" | "fmt" => Some(Family::Format),
        _ => None,
    }
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
    if name == "python" || name == "python3" || name.starts_with("python3.") {
        if let Some(family) = classify_python(args) {
            plan.add(family);
        }
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
        "cargo" => classify_cargo(args),
        "rustfmt" if args.iter().any(|arg| arg == "--check") => Some(Family::RustFmt),
        "pytest" | "py.test" => Some(Family::PyTest),
        "tox" => Some(Family::PyTox),
        "nox" => Some(Family::PyNox),
        "ruff" if args.first().is_some_and(|arg| arg == "check") => Some(Family::PyRuff),
        "mypy" => Some(Family::PyMypy),
        "pyright" => Some(Family::PyPyright),
        "pylint" => Some(Family::PyPylint),
        "black" if args.iter().any(|arg| arg == "--check") => Some(Family::PyBlack),
        "coverage" => classify_coverage(args),
        "pip" if args.first().is_some_and(|arg| arg == "install") => Some(Family::PyInstall),
        "uv" => classify_uv(args),
        "poetry" => classify_poetry(args),
        "pipenv" => classify_pipenv(args),
        "go" => classify_go(args),
        "golangci-lint" if args.first().is_some_and(|arg| arg == "run") => Some(Family::GoLint),
        "staticcheck" => Some(Family::GoStaticcheck),
        _ => None,
    };
    if let Some(family) = family {
        plan.add(family);
    }
}

fn classify_cargo(args: &[String]) -> Option<Family> {
    let mut args = args;
    while args.first().is_some_and(|arg| arg.starts_with('-')) {
        if matches!(args[0].as_str(), "--manifest-path" | "--config" | "-Z") {
            if args.len() < 2 {
                return None;
            }
            args = &args[2..];
        } else {
            args = &args[1..];
        }
    }
    match args.first()?.as_str() {
        "test" => Some(Family::RustTest),
        "nextest" if args.get(1).is_some_and(|arg| arg == "run") => Some(Family::RustNextest),
        "build" => Some(Family::RustBuild),
        "check" => Some(Family::RustCheck),
        "clippy" => Some(Family::RustClippy),
        "fmt" if args.iter().any(|arg| arg == "--check") => Some(Family::RustFmt),
        "doc" => Some(Family::RustDoc),
        _ => None,
    }
}

fn classify_python(args: &[String]) -> Option<Family> {
    if args.first().is_some_and(|arg| arg == "-m") {
        let module = args.get(1)?.as_str();
        if module == "coverage" {
            return classify_coverage(&args[2..]);
        }
        return classify_python_module(module);
    }
    None
}

fn classify_python_module(module: &str) -> Option<Family> {
    match module {
        "pytest" => Some(Family::PyTest),
        "unittest" => Some(Family::PyUnittest),
        "coverage" => Some(Family::PyCoverage),
        "pip" => Some(Family::PyInstall),
        _ => None,
    }
}

fn classify_coverage(args: &[String]) -> Option<Family> {
    match args.first()?.as_str() {
        "run" => {
            let mut options = &args[1..];
            while let Some(option) = options.first() {
                match option.as_str() {
                    "-m" => {
                        return options
                            .get(1)
                            .and_then(|module| classify_python_module(module));
                    }
                    "-a" | "-p" | "--append" | "--branch" | "--parallel-mode" => {
                        options = &options[1..];
                    }
                    // The first script and all its arguments belong to the
                    // application. Unknown options also stay raw.
                    _ => return None,
                }
            }
            None
        }
        "report" => Some(Family::PyCoverage),
        _ => None,
    }
}

fn classify_uv(args: &[String]) -> Option<Family> {
    match args.first()?.as_str() {
        "run" => {
            let mut command = &args[1..];
            while let Some(option) = command.first() {
                match option.as_str() {
                    "--" => {
                        command = &command[1..];
                        break;
                    }
                    "--with" | "--with-editable" | "--project" | "--directory" | "--python"
                    | "--env-file" => {
                        if command.get(1).is_none_or(|value| value.starts_with('-')) {
                            return None;
                        }
                        command = &command[2..];
                    }
                    "--no-project" | "--no-sync" | "--locked" | "--frozen" | "--offline"
                    | "--isolated" => command = &command[1..],
                    option
                        if [
                            "--with=",
                            "--with-editable=",
                            "--project=",
                            "--directory=",
                            "--python=",
                            "--env-file=",
                        ]
                        .iter()
                        .any(|prefix| {
                            option.starts_with(prefix) && option.len() > prefix.len()
                        }) =>
                    {
                        command = &command[1..];
                    }
                    option if option.starts_with('-') => return None,
                    _ => break,
                }
            }
            classify_python_command(command)
        }
        "pip" if args.get(1).is_some_and(|arg| arg == "install") => Some(Family::PyInstall),
        _ => None,
    }
}

fn classify_poetry(args: &[String]) -> Option<Family> {
    match args.first()?.as_str() {
        "run" => classify_python_command(&args[1..]),
        "install" => Some(Family::PyInstall),
        _ => None,
    }
}

fn classify_pipenv(args: &[String]) -> Option<Family> {
    match args.first()?.as_str() {
        "run" => classify_python_command(&args[1..]),
        "install" => Some(Family::PyInstall),
        _ => None,
    }
}

fn classify_python_command(args: &[String]) -> Option<Family> {
    let name = args
        .first()
        .and_then(|arg| Path::new(arg).file_name())
        .and_then(|arg| arg.to_str())?;
    if name == "python" || name.starts_with("python3") {
        return classify_python(&args[1..]);
    }
    let rest = &args[1..];
    match name {
        "pytest" | "py.test" => Some(Family::PyTest),
        "tox" => Some(Family::PyTox),
        "nox" => Some(Family::PyNox),
        "ruff" if rest.first().is_some_and(|arg| arg == "check") => Some(Family::PyRuff),
        "mypy" => Some(Family::PyMypy),
        "pyright" => Some(Family::PyPyright),
        "pylint" => Some(Family::PyPylint),
        "black" if rest.iter().any(|arg| arg == "--check") => Some(Family::PyBlack),
        "coverage" => classify_coverage(rest),
        "pip" if rest.first().is_some_and(|arg| arg == "install") => Some(Family::PyInstall),
        _ => None,
    }
}

fn classify_go(args: &[String]) -> Option<Family> {
    match args.first()?.as_str() {
        "test" => Some(Family::GoTest),
        "build" => Some(Family::GoBuild),
        "vet" => Some(Family::GoVet),
        "generate" => Some(Family::GoGenerate),
        _ => None,
    }
}

/// Tokenizes only shell syntax whose command boundaries are unambiguous.
fn split_shell(command: &str) -> Option<Vec<String>> {
    if command
        .chars()
        .any(|ch| (0xf0000..=0xf00ff).contains(&(ch as u32)))
    {
        return None;
    }
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut chars = command.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => word.push(chars.next()?),
            (Some(_), c @ ('&' | '|' | ';' | '<' | '>')) => word.push(escape_literal(c)),
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => quote = Some(ch),
            (None, '\\') => {
                let escaped = chars.next()?;
                if matches!(escaped, '&' | '|' | ';' | '<' | '>') {
                    word.push(escape_literal(escaped));
                } else {
                    word.push(escaped);
                }
            }
            (None, ' ' | '\t') => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            (None, '\n') => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                words.push(";".to_owned());
            }
            (None, '&' | '|' | ';' | '<' | '>') => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                let mut op = ch.to_string();
                if chars.peek() == Some(&ch) {
                    op.push(chars.next()?);
                }
                if matches!(ch, '>' | '<') && chars.peek() == Some(&'=') {
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

fn escape_literal(ch: char) -> char {
    char::from_u32(0xf0000 + ch as u32).expect("private-use unicode scalar")
}

fn restore_literal(word: &str) -> String {
    word.chars()
        .map(|ch| {
            let value = ch as u32;
            if (0xf0026..=0xf003e).contains(&value) {
                char::from_u32(value - 0xf0000).unwrap_or(ch)
            } else {
                ch
            }
        })
        .collect()
}
