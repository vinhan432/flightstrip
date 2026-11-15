//! Flightstrip CLI.
//!
//! Every subcommand is a thin shell around `flightstrip-core`: the CLI owns
//! argument parsing, the terminal, and process spawning, and nothing else.
//!
//! Arguments are parsed by hand below. `clap` is a pleasant library, but this
//! program has six subcommands and no generated shell completions, so the parser
//! is a loop over `Vec<String>` with a `match` in it — and the binary stays small.

mod cmd;
mod error;
mod interrupt;

use crate::error::{Context, Result};
use flightstrip_core::{paths, session};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
flightstrip — a flight recorder for AI coding agents.

USAGE
  flightstrip [--repo PATH] <command> [options]

COMMANDS
  watch     Record file changes until Ctrl+C.
              --title TEXT        human-readable session name
              --agent TEXT        which agent this session records (free text)
              --interval MS       how often the tree is polled (default 500)
              --no-baseline       skip hashing the repo at start (cheaper, but
                                  the first edit of an existing file has no
                                  \"before\" to diff against)
              --verbose           say why paths are skipped

  mark      Record the prompt that caused the changes that follow.
              flightstrip mark \"add a retry to the uploader\"
            Reads stdin when no text is given, or when the text is \"-\".

  exec      Run a command, record it, and parse its output for test results.
              flightstrip exec -- cargo test
            Mirrors the child's exit code.

  list      List recorded sessions in this repository.

  export    Turn a recorded session into session.json.
              [SESSION]           session id or \"latest\" (default)
              -o, --out PATH      output file (default: inside the session dir)
              --redact            replace secret-looking values
              --attribution-gap-ms MS
              --fixture           label the export as synthetic

  report    Print the markdown trust summary.
              [SESSION|FILE.json] -o, --out PATH   --redact

  help      This text.   --version   Print the version.

GLOBAL
  --repo PATH   Repository to operate on. Defaults to the nearest enclosing
                repository (a directory with .git or .flightstrip).

Typical session:
  flightstrip watch --agent \"claude cli\"
  flightstrip mark \"add a retry to the uploader\"
  flightstrip exec -- cargo test
  flightstrip export --out session.json
  flightstrip report --out session.json
";

enum Command {
    Watch {
        title: Option<String>,
        agent: Option<String>,
        interval_ms: u64,
        baseline: bool,
        verbose: bool,
    },
    Mark {
        text: Vec<String>,
    },
    Exec {
        argv: Vec<String>,
    },
    List,
    Export {
        session: String,
        out: Option<PathBuf>,
        redact: bool,
        attribution_gap_ms: u64,
        fixture: bool,
    },
    Report {
        session: String,
        out: Option<PathBuf>,
        redact: bool,
    },
    Help,
    Version,
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(argv) {
        Ok(code) => std::process::exit(code),
        Err(message) => {
            eprintln!("flightstrip: {message}");
            std::process::exit(1);
        }
    }
}

fn run(argv: Vec<String>) -> Result<i32> {
    let (repo, rest) = split_global(&argv)?;
    let command = parse(&rest)?;

    // Help and version work without a repository.
    match command {
        Command::Help => {
            print!("{HELP}");
            return Ok(0);
        }
        Command::Version => {
            println!("flightstrip {VERSION}");
            return Ok(0);
        }
        _ => {}
    }

    let root = resolve_root(repo.as_deref())?;
    if paths::is_unc(&root) {
        crate::bail!(
            "{} is a network share; a flight recorder only watches local repositories",
            root.display()
        );
    }

    match command {
        Command::Help | Command::Version => unreachable!("handled above"),

        Command::Watch {
            title,
            agent,
            interval_ms,
            baseline,
            verbose,
        } => {
            cmd::watch::run(&root, title, agent, interval_ms, baseline, verbose)?;
            Ok(0)
        }

        Command::Mark { text } => {
            let joined = if text.is_empty() || text == ["-"] {
                let mut buffer = String::new();
                use std::io::Read;
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .context("while reading the prompt from stdin")?;
                buffer
            } else {
                text.join(" ")
            };
            let text = joined.trim();
            if text.is_empty() {
                crate::bail!("a prompt needs text: flightstrip mark \"what you asked for\"");
            }
            cmd::mark::run(&root, text)?;
            Ok(0)
        }

        Command::Exec { argv } => cmd::exec_cmd::run(&root, &argv),

        Command::List => {
            cmd::list::run(&root)?;
            Ok(0)
        }

        Command::Export {
            session,
            out,
            redact,
            attribution_gap_ms,
            fixture,
        } => {
            cmd::export_cmd::run(&root, &session, out, redact, attribution_gap_ms, fixture)?;
            Ok(0)
        }

        Command::Report {
            session,
            out,
            redact,
        } => {
            cmd::report_cmd::run(&root, &session, out, redact)?;
            Ok(0)
        }
    }
}

/// Pull `--repo` out of the argument list wherever it appears, stopping at `--`
/// so a recorded command's own flags are left alone.
fn split_global(argv: &[String]) -> Result<(Option<PathBuf>, Vec<String>)> {
    let mut repo = None;
    let mut rest = Vec::new();
    let mut index = 0usize;
    while index < argv.len() {
        let arg = &argv[index];
        if arg == "--" {
            rest.extend_from_slice(&argv[index..]);
            break;
        }
        if arg == "--repo" {
            let value = argv
                .get(index + 1)
                .ok_or_else(|| "--repo needs a path".to_string())?;
            repo = Some(PathBuf::from(value));
            index += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--repo=") {
            repo = Some(PathBuf::from(value));
            index += 1;
            continue;
        }
        rest.push(arg.clone());
        index += 1;
    }
    Ok((repo, rest))
}

/// Drop the `--` that separates options from a raw command or prompt.
fn without_separator(args: &[String]) -> &[String] {
    match args.first() {
        Some(first) if first == "--" => &args[1..],
        _ => args,
    }
}

fn next_value(args: &[String], index: &mut usize, flag: &str) -> Result<String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value"))
}

fn parse(args: &[String]) -> Result<Command> {
    let Some(name) = args.first() else {
        return Ok(Command::Help);
    };
    let tail = &args[1..];

    match name.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "--version" | "-V" => Ok(Command::Version),

        "watch" => {
            let mut title = None;
            let mut agent = None;
            let mut interval_ms = 500u64;
            let mut baseline = true;
            let mut verbose = false;
            let mut index = 0usize;
            while index < tail.len() {
                match tail[index].as_str() {
                    "--title" => title = Some(next_value(tail, &mut index, "--title")?),
                    "--agent" => agent = Some(next_value(tail, &mut index, "--agent")?),
                    "--interval" => {
                        let raw = next_value(tail, &mut index, "--interval")?;
                        interval_ms = raw.parse::<u64>().map_err(|_| {
                            format!("--interval needs a number of milliseconds, not `{raw}`")
                        })?;
                    }
                    "--no-baseline" => baseline = false,
                    "--verbose" | "-v" => verbose = true,
                    "--help" | "-h" => return Ok(Command::Help),
                    other => crate::bail!("unknown option for watch: {other}"),
                }
                index += 1;
            }
            Ok(Command::Watch {
                title,
                agent,
                interval_ms,
                baseline,
                verbose,
            })
        }

        "mark" => Ok(Command::Mark {
            text: without_separator(tail).to_vec(),
        }),

        "exec" => {
            let argv: Vec<String> = without_separator(tail).to_vec();
            if argv.is_empty() {
                crate::bail!("exec needs a command: flightstrip exec -- cargo test");
            }
            Ok(Command::Exec { argv })
        }

        "list" => {
            if let Some(extra) = tail.first() {
                crate::bail!("list takes no options (got `{extra}`)");
            }
            Ok(Command::List)
        }

        "export" => {
            let mut session = None;
            let mut out = None;
            let mut redact = false;
            let mut attribution_gap_ms = flightstrip_core::attributor::DEFAULT_GAP_MS;
            let mut fixture = false;
            let mut index = 0usize;
            while index < tail.len() {
                match tail[index].as_str() {
                    "-o" | "--out" => {
                        out = Some(PathBuf::from(next_value(tail, &mut index, "--out")?))
                    }
                    "--redact" => redact = true,
                    "--fixture" => fixture = true,
                    "--attribution-gap-ms" => {
                        let raw = next_value(tail, &mut index, "--attribution-gap-ms")?;
                        attribution_gap_ms = raw.parse::<u64>().map_err(|_| {
                            format!("--attribution-gap-ms needs a number, not `{raw}`")
                        })?;
                    }
                    "--help" | "-h" => return Ok(Command::Help),
                    other if other.starts_with('-') => {
                        crate::bail!("unknown option for export: {other}")
                    }
                    other => {
                        if session.is_some() {
                            crate::bail!("export takes at most one session id");
                        }
                        session = Some(other.to_string());
                    }
                }
                index += 1;
            }
            Ok(Command::Export {
                session: session.unwrap_or_else(|| "latest".to_string()),
                out,
                redact,
                attribution_gap_ms,
                fixture,
            })
        }

        "report" => {
            let mut session = None;
            let mut out = None;
            let mut redact = false;
            let mut index = 0usize;
            while index < tail.len() {
                match tail[index].as_str() {
                    "-o" | "--out" => {
                        out = Some(PathBuf::from(next_value(tail, &mut index, "--out")?))
                    }
                    "--redact" => redact = true,
                    "--help" | "-h" => return Ok(Command::Help),
                    other if other.starts_with('-') => {
                        crate::bail!("unknown option for report: {other}")
                    }
                    other => {
                        if session.is_some() {
                            crate::bail!("report takes at most one session id or file");
                        }
                        session = Some(other.to_string());
                    }
                }
                index += 1;
            }
            Ok(Command::Report {
                session: session.unwrap_or_else(|| "latest".to_string()),
                out,
                redact,
            })
        }

        other => crate::bail!("unknown command `{other}`; try `flightstrip help`"),
    }
}

/// The nearest directory containing `.flightstrip` or `.git`, falling back to the
/// directory itself.
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

    let mut cursor = start.as_path();
    loop {
        if cursor.join(session::STATE_DIR).is_dir() || cursor.join(".git").exists() {
            return Ok(cursor.to_path_buf());
        }
        match cursor.parent() {
            Some(parent) => cursor = parent,
            None => return Ok(start),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn root_resolution_walks_up_to_the_repo_marker() {
        let base = std::env::temp_dir().join(format!("fs-cli-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("repo").join(".git")).unwrap();
        std::fs::create_dir_all(base.join("repo").join("src").join("deep")).unwrap();
        let found = resolve_root(Some(&base.join("repo").join("src").join("deep"))).unwrap();
        assert!(found.ends_with("repo"), "{}", found.display());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn global_repo_flag_is_lifted_out() {
        let (repo, rest) = split_global(&args(&[
            "--repo", "C:/repo", "export", "--redact", "--out", "x.json",
        ]))
        .unwrap();
        assert_eq!(repo.unwrap().to_string_lossy(), "C:/repo");
        assert_eq!(rest, args(&["export", "--redact", "--out", "x.json"]));
    }

    #[test]
    fn repo_flag_after_the_separator_belongs_to_the_command() {
        let (repo, rest) = split_global(&args(&["exec", "--", "tool", "--repo", "x"])).unwrap();
        assert!(repo.is_none());
        assert_eq!(rest, args(&["exec", "--", "tool", "--repo", "x"]));
    }

    #[test]
    fn exec_keeps_hyphenated_arguments() {
        let command = parse(&args(&["exec", "--", "cargo", "test", "--all", "-q"])).unwrap();
        let Command::Exec { argv } = command else {
            panic!("expected exec");
        };
        assert_eq!(argv, args(&["cargo", "test", "--all", "-q"]));
    }

    #[test]
    fn export_defaults_to_the_latest_session() {
        let command = parse(&args(&["export"])).unwrap();
        let Command::Export {
            session, redact, ..
        } = command
        else {
            panic!("expected export");
        };
        assert_eq!(session, "latest");
        assert!(!redact);
    }

    #[test]
    fn unknown_options_are_rejected() {
        assert!(parse(&args(&["watch", "--debounced"])).is_err());
        assert!(parse(&args(&["nonsense"])).is_err());
        assert!(parse(&args(&["--interval"])).is_err());
    }

    #[test]
    fn no_arguments_prints_help() {
        assert!(matches!(parse(&[]).unwrap(), Command::Help));
    }
}
