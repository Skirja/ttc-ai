//! Parse for understanding only: execution always uses the original invocation.
use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Shell {
    #[default]
    Posix,
    PowerShell,
    Cmd,
}
#[derive(Debug, Clone)]
pub enum Invocation {
    Argv(Vec<OsString>),
    Shell {
        executable: OsString,
        dialect: Shell,
        command: String,
    },
}
impl Invocation {
    pub fn display(&self) -> String {
        match self {
            Self::Argv(v) => v
                .iter()
                .map(|s| quote(&s.to_string_lossy()))
                .collect::<Vec<_>>()
                .join(" "),
            Self::Shell { command, .. } => command.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandAst {
    pub source: String,
    pub shell: Shell,
    pub commands: Vec<SimpleCommand>,
    pub operators: Vec<String>,
    pub uncertain: bool,
    pub exact_output: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleCommand {
    pub argv: Vec<String>,
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadGraph {
    pub label: String,
    pub cwd: PathBuf,
    pub family: Option<String>,
    pub confidence: Confidence,
    pub children: Vec<WorkloadGraph>,
    pub notes: Vec<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Confidence {
    ExactDirectTool,
    ResolvedProjectScript,
    ResolvedNestedWrapper,
    Unknown,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    ReadOnly,
    Diagnostic,
    Mutating,
    Destructive,
    Interactive,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Classification {
    pub schema_version: u32,
    pub ast: CommandAst,
    pub workload: WorkloadGraph,
    pub risk: Risk,
    pub families: Vec<String>,
    pub filterable: bool,
    pub hook_action: String,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Channel {
    Stdout,
    Stderr,
}
#[derive(Debug, Clone)]
pub struct OutputEvent {
    pub channel: Channel,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessOutcome {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// Facts observed from the child process TTC actually spawned.
///
/// Static workload graph nodes are intentionally not represented as executed stages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionEvidence {
    pub schema_version: u32,
    pub stages: Vec<StageEvidence>,
    pub nested_stage_attribution: NestedStageAttribution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NestedStageAttribution {
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageEvidence {
    pub stage_id: u32,
    pub kind: StageKind,
    pub started: bool,
    pub finished: bool,
    pub outcome: Option<ProcessOutcome>,
    pub duration_ms: u64,
    pub stdout: StreamEvidence,
    pub stderr: StreamEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    TopLevelProcess,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamEvidence {
    pub events: u64,
    pub bytes: u64,
}
impl ProcessOutcome {
    pub fn success(&self) -> bool {
        self.code == Some(0) && self.signal.is_none()
    }
    pub fn from_status(s: std::process::ExitStatus) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            Self {
                code: s.code(),
                signal: s.signal(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                code: s.code(),
                signal: None,
            }
        }
    }
}

impl ExecutionEvidence {
    pub fn top_level_started() -> Self {
        Self {
            schema_version: 1,
            stages: vec![StageEvidence {
                stage_id: 0,
                kind: StageKind::TopLevelProcess,
                started: true,
                finished: false,
                outcome: None,
                duration_ms: 0,
                stdout: StreamEvidence::default(),
                stderr: StreamEvidence::default(),
            }],
            nested_stage_attribution: NestedStageAttribution::Unavailable,
        }
    }

    pub fn observe(&mut self, event: &OutputEvent) {
        let Some(stage) = self.stages.first_mut() else {
            return;
        };
        let stream = match event.channel {
            Channel::Stdout => &mut stage.stdout,
            Channel::Stderr => &mut stage.stderr,
        };
        stream.events = stream.events.saturating_add(1);
        stream.bytes = stream.bytes.saturating_add(event.bytes.len() as u64);
    }

    pub fn finish(&mut self, outcome: ProcessOutcome, duration_ms: u64) {
        let Some(stage) = self.stages.first_mut() else {
            return;
        };
        stage.finished = true;
        stage.outcome = Some(outcome);
        stage.duration_ms = duration_ms;
    }
}
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
pub fn parse(source: &str, shell: Shell) -> CommandAst {
    let mut ast = CommandAst {
        source: source.into(),
        shell,
        commands: vec![],
        operators: vec![],
        uncertain: false,
        exact_output: false,
    };
    if source.len() > 64 * 1024
        || source
            .chars()
            .any(|c| c.is_control() && !['\t', '\n', '\r'].contains(&c))
    {
        ast.uncertain = true;
        return ast;
    }
    if shell != Shell::Posix {
        // Windows has its own deliberately narrow literal grammar. No POSIX decoding.
        if source.chars().any(|c| "\"'`$%&|<>^();\n\r".contains(c)) {
            ast.uncertain = true;
            return ast;
        }
        let argv = source.split_ascii_whitespace().map(str::to_owned).collect();
        ast.commands.push(SimpleCommand {
            argv,
            start: 0,
            end: source.len(),
        });
        return ast;
    }
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .is_err()
    {
        ast.uncertain = true;
        return ast;
    }
    let started = std::time::Instant::now();
    let mut cancelled =
        |_: &tree_sitter::ParseState| started.elapsed() > std::time::Duration::from_millis(50);
    let options = tree_sitter::ParseOptions::new().progress_callback(&mut cancelled);
    let Some(tree) = parser.parse_with_options(
        &mut |byte, _| &source.as_bytes()[byte..],
        None,
        Some(options),
    ) else {
        ast.uncertain = true;
        return ast;
    };
    ast.uncertain = tree.root_node().has_error();
    fn visit(n: tree_sitter::Node<'_>, src: &str, ast: &mut CommandAst, depth: usize) {
        if depth > 128 || ast.commands.len() > 4096 {
            ast.uncertain = true;
            return;
        }
        match n.kind() {
            "command" => {
                let text = &src[n.byte_range()];
                match shell_words::split(text) {
                    Ok(argv) => ast.commands.push(SimpleCommand {
                        argv,
                        start: n.start_byte(),
                        end: n.end_byte(),
                    }),
                    Err(_) => ast.uncertain = true,
                }
            }
            "pipeline" | "redirected_statement" | "file_redirect" | "heredoc_redirect" => {
                ast.exact_output = true
            }
            "command_substitution"
            | "process_substitution"
            | "expansion"
            | "simple_expansion"
            | "arithmetic_expansion"
            | "function_definition"
            | "for_statement"
            | "while_statement"
            | "if_statement"
            | "case_statement" => ast.uncertain = true,
            "&&" | "||" | ";" | "|" => ast.operators.push(n.kind().into()),
            "&" => ast.uncertain = true,
            _ => {}
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            visit(child, src, ast, depth + 1);
        }
    }
    visit(tree.root_node(), source, &mut ast, 0);
    if ast.commands.is_empty() {
        ast.uncertain = true;
    }
    ast
}
