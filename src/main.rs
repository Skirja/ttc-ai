use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
};
use ttc_ai::{
    command::{Channel, Invocation, Shell},
    config, execution, integrations,
    recovery::Store,
    resolver,
};
#[derive(Parser)]
#[command(
    name = "ttc",
    version,
    about = "Token Tight Command — semantic output filtering with local recall"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Run the original command once. Use --shell and --command for shell text.
    Run(Run),
    /// Execute without capture or filtering.
    Raw(Run),
    /// Explain project scripts, nested workloads and conservative filter decisions.
    Explain {
        command: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        cwd: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "posix")]
        dialect: Dialect,
    },
    /// Retrieve captured output without rerunning the command.
    Recall {
        id: String,
        #[arg(long, conflicts_with = "tail")]
        head: Option<usize>,
        #[arg(long)]
        tail: Option<usize>,
        #[arg(long)]
        grep: Option<String>,
        #[arg(long)]
        lines: Option<String>,
        #[arg(long, conflicts_with = "stderr")]
        stdout: bool,
        #[arg(long)]
        stderr: bool,
        #[arg(long)]
        raw: bool,
    },
    /// Estimated local context savings, never provider billing.
    Gain {
        #[arg(long)]
        today: bool,
        #[arg(long)]
        history: bool,
        #[arg(long)]
        json: bool,
    },
    Doctor,
    Cache {
        #[command(subcommand)]
        command: Cache,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Install {
        #[arg(value_parser=["codex"])]
        harness: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, hide = true)]
        home: Option<PathBuf>,
    },
    Uninstall {
        #[arg(value_parser=["codex"])]
        harness: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, hide = true)]
        home: Option<PathBuf>,
    },
    Hook {
        #[arg(value_parser=["codex"])]
        harness: String,
    },
}
#[derive(Args)]
struct Run {
    #[arg(long, requires = "command")]
    shell: Option<OsString>,
    #[arg(long, requires = "shell")]
    login: bool,
    #[arg(long, requires = "shell", conflicts_with = "argv")]
    command: Option<String>,
    #[arg(long, value_enum, default_value = "posix")]
    dialect: Dialect,
    #[arg(long)]
    cwd: Option<PathBuf>,
    #[arg(last = true, required_unless_present = "command")]
    argv: Vec<OsString>,
}
#[derive(Clone, Copy, ValueEnum)]
enum Dialect {
    Posix,
    Powershell,
    Cmd,
}
impl From<Dialect> for Shell {
    fn from(v: Dialect) -> Self {
        match v {
            Dialect::Posix => Shell::Posix,
            Dialect::Powershell => Shell::PowerShell,
            Dialect::Cmd => Shell::Cmd,
        }
    }
}
#[derive(Subcommand)]
enum Cache {
    Status,
    Clean {
        #[arg(long)]
        all: bool,
    },
}
#[derive(Subcommand)]
enum ConfigCommand {
    Show,
}
fn print_json(v: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}
fn main() {
    if let Err(e) = app() {
        eprintln!("ttc: {e:#}");
        std::process::exit(1);
    }
}
fn app() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    match cli.command {
        Commands::Run(r) => run(r, false),
        Commands::Raw(r) => run(r, true),
        Commands::Explain {
            command,
            json,
            cwd: dir,
            dialect,
        } => {
            let c = resolver::classify_cached(
                &command,
                dir.as_deref().unwrap_or(&cwd),
                dialect.into(),
                &config::data_dir().join("discovery"),
            );
            if json {
                print_json(&c)
            } else {
                println!(
                    "risk: {:?}\nfilterable: {}\nfamilies: {}\nhook: {}",
                    c.risk,
                    c.filterable,
                    c.families.join(", "),
                    c.hook_action
                );
                fn graph(n: &ttc_ai::command::WorkloadGraph, d: usize) {
                    println!(
                        "{}{} [{}] {:?}",
                        "  ".repeat(d),
                        n.label,
                        n.cwd.display(),
                        n.confidence
                    );
                    for note in &n.notes {
                        println!("{}  {note}", "  ".repeat(d));
                    }
                    for c in &n.children {
                        graph(c, d + 1);
                    }
                }
                graph(&c.workload, 0);
                for r in c.reasons {
                    println!("{r}");
                }
                Ok(())
            }
        }
        Commands::Recall {
            id,
            head,
            tail,
            grep,
            lines,
            stdout,
            stderr,
            raw,
        } => {
            let store = Store::open()?;
            let mut safe = SafeOutput::default();
            let record = store.record(&id)?;
            if record.partial {
                eprintln!("TTC: partial capture; uncaptured output was forwarded raw");
            }
            let selective = head.is_some() || tail.is_some() || grep.is_some() || lines.is_some();
            if !selective {
                for e in store.reader(&id)? {
                    let e = e?;
                    if (stdout && e.channel != Channel::Stdout)
                        || (stderr && e.channel != Channel::Stderr)
                    {
                        continue;
                    }
                    if raw {
                        std::io::stdout().write_all(&e.bytes)?;
                    } else {
                        safe.write(&e.bytes)?;
                    }
                }
                safe.finish()?;
                return Ok(());
            }
            let (start, end) = if let Some(s) = lines {
                let (a, b) = s.split_once(':').context("--lines must be START:END")?;
                let a = a.parse::<usize>()?;
                let b = b.parse::<usize>()?;
                if a == 0 || b < a {
                    bail!("invalid line range");
                }
                (a, b)
            } else {
                (1, usize::MAX)
            };
            let mut pending = Vec::new();
            let mut index = 0;
            let mut count = 0;
            let mut queue = std::collections::VecDeque::new();
            let mut output = |line: Vec<u8>| -> Result<()> {
                index += 1;
                if index < start || index > end {
                    return Ok(());
                }
                if grep
                    .as_ref()
                    .is_some_and(|p| !String::from_utf8_lossy(&line).contains(p))
                {
                    return Ok(());
                }
                if head.is_some_and(|n| count >= n) {
                    return Ok(());
                }
                count += 1;
                if let Some(n) = tail {
                    queue.push_back(line);
                    while queue.len() > n {
                        queue.pop_front();
                    }
                } else if raw {
                    std::io::stdout().write_all(&line)?;
                } else {
                    safe.write(&line)?;
                }
                Ok(())
            };
            for e in store.reader(&id)? {
                let e = e?;
                if (stdout && e.channel != Channel::Stdout)
                    || (stderr && e.channel != Channel::Stderr)
                {
                    continue;
                }
                pending.extend(e.bytes);
                while let Some(i) = pending.iter().position(|b| *b == b'\n') {
                    output(pending.drain(..=i).collect())?;
                }
                if pending.len() > 4 * 1024 * 1024 {
                    bail!("line exceeds 4 MiB; use unselected recall --raw");
                }
            }
            if !pending.is_empty() {
                output(pending)?;
            }
            for line in queue {
                if raw {
                    std::io::stdout().write_all(&line)?;
                } else {
                    safe.write(&line)?;
                }
            }
            safe.finish()?;
            Ok(())
        }
        Commands::Gain {
            today,
            history,
            json: _,
        } => print_json(&Store::open()?.gain(today, history)?),
        Commands::Doctor => print_json(&integrations::doctor()),
        Commands::Cache { command } => {
            let s = Store::open()?;
            match command {
                Cache::Status => print_json(&s.status()?),
                Cache::Clean { all } => {
                    println!("removed {} captures", s.clean(&config::load(&cwd)?, all)?);
                    Ok(())
                }
            }
        }
        Commands::Config {
            command: ConfigCommand::Show,
        } => {
            println!("{}", toml::to_string_pretty(&config::load(&cwd)?)?);
            Ok(())
        }
        Commands::Install {
            harness: _,
            dry_run,
            home,
        } => print_json(&integrations::install(
            &home.unwrap_or_else(config::codex_home),
            &std::env::current_exe()?,
            dry_run,
        )?),
        Commands::Uninstall {
            harness: _,
            dry_run,
            home,
        } => print_json(&integrations::uninstall(
            &home.unwrap_or_else(config::codex_home),
            dry_run,
        )?),
        Commands::Hook { harness: _ } => {
            let mut s = String::new();
            if std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_string(&mut s)
                .is_err()
                || s.len() > 1024 * 1024
            {
                println!("{{}}");
                return Ok(());
            }
            let v = integrations::hook(&s, &std::env::current_exe()?);
            println!("{}", serde_json::to_string(&v)?);
            Ok(())
        }
    }
}
#[derive(Default)]
struct SafeOutput {
    pending: Vec<u8>,
}
impl SafeOutput {
    fn text(s: &str) -> Result<()> {
        let mut out = std::io::stdout().lock();
        for c in s.chars() {
            if c.is_control() && !['\n', '\r', '\t'].contains(&c) {
                write!(out, "\\u{{{:x}}}", c as u32)?;
            } else {
                write!(out, "{c}")?;
            }
        }
        Ok(())
    }
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.pending.extend_from_slice(bytes);
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    Self::text(s)?;
                    self.pending.clear();
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    Self::text(std::str::from_utf8(&self.pending[..valid])?)?;
                    let invalid = e.error_len();
                    self.pending.drain(..valid);
                    if let Some(n) = invalid {
                        for b in self.pending.drain(..n) {
                            write!(std::io::stdout(), "\\x{b:02x}")?;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        for b in self.pending.drain(..) {
            write!(std::io::stdout(), "\\x{b:02x}")?;
        }
        Ok(())
    }
}
fn run(r: Run, raw: bool) -> Result<()> {
    let cwd = r.cwd.unwrap_or(std::env::current_dir()?);
    let cfg = config::load(&cwd).unwrap_or_else(|e| {
        eprintln!("TTC config invalid ({e}); passthrough");
        config::Config {
            enabled: false,
            ..Default::default()
        }
    });
    let inv = if let (Some(executable), Some(command)) = (r.shell, r.command) {
        Invocation::Shell {
            executable,
            dialect: r.dialect.into(),
            login: r.login,
            command,
        }
    } else {
        Invocation::Argv(r.argv)
    };
    execution::exit(execution::run(inv, &cwd, &cfg, raw)?)
}
