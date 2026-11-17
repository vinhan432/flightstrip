//! Flightstrip CLI.
//!
//! Every subcommand is a thin shell around `flightstrip-core`: the CLI owns
//! argument parsing, the terminal, and process spawning, and nothing else.

mod cmd;
mod config;
mod error;
mod interrupt;
mod style;

use crate::error::{Context, Result};
use crate::style::Style;
use clap::{Args, Parser, Subcommand, ValueEnum};
use flightstrip_core::schema::{SessionLabel, Severity};
use flightstrip_core::{paths, session};
use std::path::{Path, PathBuf};

const AFTER_HELP: &str = "\
EXAMPLES
  flightstrip init
  flightstrip watch --agent \"claude cli\"
  flightstrip mark \"add a retry to the uploader\"
  flightstrip exec -- cargo test
  flightstrip export --out session.json
  flightstrip verify session.json
  flightstrip report session.json --format github --fail-on critical

Flightstrip records what an agent changed and what it ran. Prompt attribution is
heuristic; detectors are deterministic rules, not a sandbox or a verdict.";

#[derive(Debug, Parser)]
#[command(
    name = "flightstrip",
    version,
    about = "A flight recorder for AI coding agents.",
    long_about = "Record a coding session, attribute changes to prompts, run deterministic \
detectors, and export a session.json that can be reviewed or shared.",
    after_help = AFTER_HELP,
    arg_required_else_help = true
)]
struct Cli {
    /// Repository to operate on. Defaults to the nearest enclosing repository.
    #[arg(long, global = true, value_name = "PATH")]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Record file changes until Ctrl+C.
    Watch(WatchArgs),
    /// Record the prompt that caused the changes that follow.
    Mark(MarkArgs),
    /// Run a command, record it, and parse its output for test results.
    Exec(ExecArgs),
    /// List recorded sessions in this repository.
    List(ListArgs),
    /// Turn a recorded session into session.json.
    Export(ExportArgs),
    /// Print the trust summary for a session or a session.json file.
    Report(ReportArgs),
    /// Create .flightstrip/config.toml and add .flightstrip/ to an existing
    /// .gitignore.
    Init(InitArgs),
    /// Show the active session, uptime, counts and recent events.
    Status(StatusArgs),
    /// Check the environment and the project configuration.
    Doctor(DoctorArgs),
    /// Delete old sessions and orphaned snapshot objects.
    Prune(PruneArgs),
    /// Print shell completions.
    Completions(CompletionsArgs),
    /// Print a shell snippet defining fsmark and fsx. Does not edit profiles.
    #[command(name = "shell-init")]
    ShellInit(ShellInitArgs),
    /// Validate a session.json against the export contract.
    Verify(VerifyArgs),
}

#[derive(Debug, Args)]
struct WatchArgs {
    /// Human-readable session name.
    #[arg(long, value_name = "TEXT")]
    title: Option<String>,
    /// Which agent this session records (free text).
    #[arg(long, value_name = "TEXT")]
    agent: Option<String>,
    /// How often the tree is polled, in milliseconds. Defaults to config.toml.
    #[arg(long, value_name = "MS")]
    interval: Option<u64>,
    /// Skip hashing the repository at start (cheaper, but the first edit has no
    /// before state to diff against).
    #[arg(long)]
    no_baseline: bool,
    /// Say why paths are skipped.
    #[arg(long, short)]
    verbose: bool,
}

#[derive(Debug, Args)]
struct MarkArgs {
    /// Prompt text. Reads stdin when omitted.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    text: Vec<String>,
    /// Read the prompt from stdin even when text is present.
    #[arg(long)]
    stdin: bool,
    /// Tag this prompt with an agent name.
    #[arg(long, value_name = "NAME")]
    agent: Option<String>,
}

#[derive(Debug, Args)]
struct ExecArgs {
    /// Command to run, including its arguments. Use `--` if it starts with `-`.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = true,
        num_args = 1..,
        value_name = "COMMAND"
    )]
    argv: Vec<String>,
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Machine-readable JSON.
    #[arg(long, conflicts_with = "quiet")]
    json: bool,
    /// Print one session id per line.
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct ExportArgs {
    /// Session id, or "latest" (default).
    #[arg(value_name = "SESSION")]
    session: Option<String>,
    /// Output file (default: inside the session directory).
    #[arg(short, long, value_name = "PATH")]
    out: Option<PathBuf>,
    /// Replace secret-looking values. On by default when config sets
    /// redact_by_default.
    #[arg(long)]
    redact: bool,
    /// Attribution window in milliseconds.
    #[arg(long, value_name = "MS")]
    attribution_gap_ms: Option<u64>,
    /// Omit diff hunks from the export.
    #[arg(long)]
    no_hunks: bool,
    /// Cap lines per diff hunk.
    #[arg(long, value_name = "N")]
    max_hunk_lines: Option<usize>,
    /// Mark the export as a fixture rather than a recording.
    #[arg(long, value_name = "KIND")]
    label: Option<LabelArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LabelArg {
    Recorded,
    Fixture,
}

impl LabelArg {
    fn into_label(self) -> SessionLabel {
        match self {
            LabelArg::Recorded => SessionLabel::Recorded,
            LabelArg::Fixture => SessionLabel::Fixture,
        }
    }
}

#[derive(Debug, Args)]
struct ReportArgs {
    /// Session id or a path to session.json. Defaults to "latest".
    #[arg(value_name = "SESSION")]
    session: Option<String>,
    /// Write the report to a file instead of only stdout.
    #[arg(short, long, value_name = "PATH")]
    out: Option<PathBuf>,
    /// Redact secret-looking values.
    #[arg(long)]
    redact: bool,
    /// Report format.
    #[arg(long, value_enum, default_value_t = ReportFormat::Md)]
    format: ReportFormat,
    /// Exit with code 2 when an incident at or above this severity exists.
    #[arg(long, value_enum, value_name = "SEVERITY")]
    fail_on: Option<SeverityArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ReportFormat {
    Md,
    Github,
    Json,
}

impl ReportFormat {
    fn into_format(self) -> cmd::report_cmd::Format {
        match self {
            ReportFormat::Md => cmd::report_cmd::Format::Markdown,
            ReportFormat::Github => cmd::report_cmd::Format::Github,
            ReportFormat::Json => cmd::report_cmd::Format::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SeverityArg {
    Critical,
    Warn,
    Info,
}

impl SeverityArg {
    fn into_severity(self) -> Severity {
        match self {
            SeverityArg::Critical => Severity::Critical,
            SeverityArg::Warn => Severity::Warn,
            SeverityArg::Info => Severity::Info,
        }
    }
}

#[derive(Debug, Args)]
struct InitArgs {
    /// Overwrite an existing config.toml.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct StatusArgs {
    /// Machine-readable JSON.
    #[arg(long, conflicts_with = "quiet")]
    json: bool,
    /// One line: id, uptime, event count.
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    /// Machine-readable JSON.
    #[arg(long, conflicts_with = "quiet")]
    json: bool,
    /// Print only failures and warnings.
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct PruneArgs {
    /// Keep the newest N sessions (default 20).
    #[arg(long, value_name = "N", default_value_t = 20)]
    keep: usize,
    /// Also delete sessions older than this many days.
    #[arg(long, value_name = "DAYS")]
    older_than: Option<u64>,
    /// Show what would be deleted without deleting anything.
    #[arg(long)]
    dry_run: bool,
    /// Do not ask for confirmation.
    #[arg(long, short = 'y')]
    yes: bool,
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    shell: ShellArg,
}

#[derive(Debug, Args)]
struct ShellInitArgs {
    /// Shell snippet to print.
    #[arg(value_enum)]
    shell: ShellArg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ShellArg {
    Powershell,
    Bash,
    Zsh,
}

impl ShellArg {
    fn completion_shell(self) -> clap_complete::Shell {
        match self {
            ShellArg::Powershell => clap_complete::Shell::PowerShell,
            ShellArg::Bash => clap_complete::Shell::Bash,
            ShellArg::Zsh => clap_complete::Shell::Zsh,
        }
    }
}

#[derive(Debug, Args)]
struct VerifyArgs {
    /// Path to a session.json file.
    #[arg(value_name = "SESSION.JSON")]
    path: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            let style = Style::auto();
            eprintln!("{} {message}", style.fail("error:"));
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    // These do not need a repository.
    match &cli.command {
        Command::Completions(args) => {
            cmd::completions::run(args.shell.completion_shell());
            return Ok(0);
        }
        Command::ShellInit(args) => {
            cmd::shell_init::run(args.shell);
            return Ok(0);
        }
        Command::Verify(args) => {
            let ok = cmd::verify::run(&args.path, Style::auto())?;
            return Ok(if ok { 0 } else { 1 });
        }
        _ => {}
    }

    let root = resolve_root(cli.repo.as_deref())?;
    if paths::is_unc(&root) {
        crate::bail!(
            "{} is a network share; a flight recorder only watches local repositories. \
             Move the repository to a local drive or pass a local --repo path.",
            root.display()
        );
    }
    let style = Style::auto();

    match cli.command {
        Command::Completions(_) | Command::ShellInit(_) | Command::Verify(_) => {
            unreachable!("handled above")
        }

        Command::Watch(args) => {
            let cfg = config::Config::load(&root)?;
            cmd::watch::run(
                &root,
                cmd::watch::Settings {
                    title: args.title,
                    agent: args.agent,
                    interval_ms: args.interval.unwrap_or(cfg.poll_ms),
                    baseline: !args.no_baseline,
                    verbose: args.verbose,
                },
                cfg,
                style,
            )?;
            Ok(0)
        }

        Command::Mark(args) => {
            let joined = if args.stdin || args.text.is_empty() {
                let mut buffer = String::new();
                use std::io::Read;
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .context("while reading the prompt from stdin")?;
                buffer
            } else {
                args.text.join(" ")
            };
            let text = joined.trim();
            if text.is_empty() {
                crate::bail!(
                    "a prompt needs text: flightstrip mark \"what you asked for\" \
                     (or pipe it with --stdin)"
                );
            }
            cmd::mark::run(&root, text, args.agent, style)?;
            Ok(0)
        }

        Command::Exec(args) => cmd::exec_cmd::run(&root, &args.argv, style),

        Command::List(args) => {
            let cfg = config::Config::load(&root)?;
            cmd::list::run(&root, &cfg, args.json, args.quiet, style)?;
            Ok(0)
        }

        Command::Export(args) => {
            let cfg = config::Config::load(&root)?;
            let gap = args
                .attribution_gap_ms
                .unwrap_or(cfg.attribution_decay_minutes.saturating_mul(60_000));
            cmd::export_cmd::run(
                &root,
                args.session.as_deref().unwrap_or("latest"),
                args.out,
                args.redact || cfg.redact_by_default,
                gap,
                args.label.map(LabelArg::into_label),
                args.no_hunks,
                args.max_hunk_lines,
                &cfg,
                style,
            )?;
            Ok(0)
        }

        Command::Report(args) => {
            let cfg = config::Config::load(&root)?;
            let gated = cmd::report_cmd::run(
                &root,
                args.session.as_deref().unwrap_or("latest"),
                args.out,
                args.redact || cfg.redact_by_default,
                args.format.into_format(),
                args.fail_on.map(SeverityArg::into_severity),
                &cfg,
                style,
            )?;
            Ok(if gated { 2 } else { 0 })
        }

        Command::Init(args) => {
            cmd::init::run(&root, args.force, style)?;
            Ok(0)
        }

        Command::Status(args) => {
            cmd::status::run(&root, args.json, args.quiet, style)?;
            Ok(0)
        }

        Command::Doctor(args) => {
            let ok = cmd::doctor::run(&root, args.json, args.quiet, style)?;
            Ok(if ok { 0 } else { 1 })
        }

        Command::Prune(args) => {
            cmd::prune::run(
                &root,
                args.keep,
                args.older_than,
                args.dry_run,
                args.yes,
                style,
            )?;
            Ok(0)
        }
    }
}

/// The nearest directory containing `.flightstrip` or `.git`, falling back to the
/// directory itself. An explicit `--repo` is used as given.
fn resolve_root(explicit: Option<&Path>) -> Result<PathBuf> {
    let start = match explicit {
        Some(path) => {
            let abs = std::fs::canonicalize(path)
                .with_context(|| format!("no such directory: {}", path.display()))?;
            paths::clean_path(&abs)
        }
        None => {
            let cwd = std::env::current_dir().context("while reading the current directory")?;
            paths::clean_path(&std::fs::canonicalize(&cwd).unwrap_or(cwd))
        }
    };
    if !start.is_dir() {
        crate::bail!("{} is not a directory", start.display());
    }

    // An explicit `--repo` is authoritative. Walking up from it would silently
    // record the enclosing repository when the user pointed at a nested one
    // (a test fixture, a submodule, a scratch repo inside this project).
    if explicit.is_some() {
        return Ok(start);
    }
    Ok(find_root(start))
}

/// The nearest ancestor with `.flightstrip` or `.git`, falling back to `start`.
fn find_root(start: PathBuf) -> PathBuf {
    let mut cursor = start.as_path();
    loop {
        if cursor.join(session::STATE_DIR).is_dir() || cursor.join(".git").exists() {
            return cursor.to_path_buf();
        }
        match cursor.parent() {
            Some(parent) => cursor = parent,
            None => return start,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> std::result::Result<Cli, clap::Error> {
        let mut full = vec!["flightstrip"];
        full.extend_from_slice(args);
        Cli::try_parse_from(full)
    }

    #[test]
    fn root_resolution_walks_up_to_the_repo_marker() {
        let base = std::env::temp_dir().join(format!("fs-cli-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("repo").join(".git")).unwrap();
        std::fs::create_dir_all(base.join("repo").join("src").join("deep")).unwrap();
        let found = find_root(paths::clean_path(
            &base.join("repo").join("src").join("deep"),
        ));
        assert!(found.ends_with("repo"), "{}", found.display());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn an_explicit_repo_does_not_walk_up_into_an_enclosing_repo() {
        let base = std::env::temp_dir().join(format!("fs-cli-nested-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("outer").join(".git")).unwrap();
        std::fs::create_dir_all(base.join("outer").join("inner")).unwrap();
        let found = resolve_root(Some(&base.join("outer").join("inner"))).unwrap();
        assert!(found.ends_with("inner"), "{}", found.display());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn global_repo_flag_is_accepted_before_or_after_the_command() {
        let cli = parse(&["--repo", "C:/repo", "list"]).unwrap();
        assert_eq!(cli.repo.unwrap().to_string_lossy(), "C:/repo");
        let cli = parse(&["list", "--repo", "C:/repo"]).unwrap();
        assert_eq!(cli.repo.unwrap().to_string_lossy(), "C:/repo");
    }

    #[test]
    fn exec_keeps_hyphenated_arguments() {
        let cli = parse(&["exec", "--", "cargo", "test", "--all", "-q"]).unwrap();
        let Command::Exec(args) = cli.command else {
            panic!("expected exec");
        };
        assert_eq!(args.argv, vec!["cargo", "test", "--all", "-q"]);
    }

    #[test]
    fn exec_works_without_the_separator() {
        let cli = parse(&["exec", "echo", "hello"]).unwrap();
        let Command::Exec(args) = cli.command else {
            panic!("expected exec");
        };
        assert_eq!(args.argv, vec!["echo", "hello"]);
    }

    #[test]
    fn export_defaults_to_the_latest_session() {
        let cli = parse(&["export"]).unwrap();
        let Command::Export(args) = cli.command else {
            panic!("expected export");
        };
        assert!(args.session.is_none());
        assert!(!args.redact);
        assert!(!args.no_hunks);
    }

    #[test]
    fn report_formats_and_fail_on_parse() {
        let cli = parse(&[
            "report",
            "session.json",
            "--format",
            "github",
            "--fail-on",
            "critical",
        ])
        .unwrap();
        let Command::Report(args) = cli.command else {
            panic!("expected report");
        };
        assert_eq!(args.format, ReportFormat::Github);
        assert_eq!(args.fail_on, Some(SeverityArg::Critical));
    }

    #[test]
    fn mark_accepts_an_agent_tag() {
        let cli = parse(&["mark", "--agent", "claude", "do the thing"]).unwrap();
        let Command::Mark(args) = cli.command else {
            panic!("expected mark");
        };
        assert_eq!(args.agent.as_deref(), Some("claude"));
        assert_eq!(args.text, vec!["do the thing"]);
    }

    #[test]
    fn unknown_commands_and_options_fail() {
        assert!(parse(&["nonsense"]).is_err());
        assert!(parse(&["watch", "--debounced"]).is_err());
    }

    #[test]
    fn no_arguments_asks_for_a_command() {
        assert!(Cli::try_parse_from(["flightstrip"]).is_err());
    }

    #[test]
    fn completions_accepts_the_three_supported_shells() {
        for shell in ["powershell", "bash", "zsh"] {
            assert!(parse(&["completions", shell]).is_ok(), "{shell}");
        }
    }
}
