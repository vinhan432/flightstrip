//! `flightstrip watch`: the recording loop.

use crate::config::Config;
use crate::error::{Context, Result};
use crate::interrupt;
use crate::style::Style;
use flightstrip_core::paths;
use flightstrip_core::scan::{Recorder, WatchOptions};
use flightstrip_core::schema::{EventKind, SessionLabel};
use flightstrip_core::session::{self, EventLog};
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// What the CLI asked for, separate from the project config.
pub struct Settings {
    pub title: Option<String>,
    pub agent: Option<String>,
    pub interval_ms: u64,
    pub baseline: bool,
    pub verbose: bool,
}

pub fn run(root: &Path, settings: Settings, config: Config, style: Style) -> Result<()> {
    // Refuse to run twice on one repo: two recorders would interleave events.
    if let Some(active) = session::read_active(root) {
        if session::pid_alive(active.pid) {
            crate::bail!(
                "a session is already recording ({}, pid {}); stop it with Ctrl+C first",
                active.id,
                active.pid
            );
        }
        eprintln!(
            "warning: cleared a stale session pointer left by pid {}",
            active.pid
        );
        session::clear_active(root);
    }

    let title = settings.title.clone().unwrap_or_else(|| {
        let root_name = session::repo_name(root);
        format!("{root_name} session")
    });

    interrupt::install().context("while installing the Ctrl+C handler")?;
    record(root, &title, &settings, &config, style, interrupt::flag())?;
    Ok(())
}

/// The part that owns the session lifecycle. Split out from [`run`] so a test
/// can stop it with an already-set flag and prove that the active pointer is
/// removed, which is the part of Ctrl+C handling that matters on disk.
fn record(
    root: &Path,
    title: &str,
    settings: &Settings,
    config: &Config,
    style: Style,
    stop: &AtomicBool,
) -> Result<()> {
    let mut log = EventLog::create(root, title, settings.agent.clone(), SessionLabel::Recorded)
        .context("while starting the session")?;
    let id = log.id();
    session::write_active(root, &id).context("while claiming the active session")?;

    println!("{}", style.bold("Flightstrip recorder"));
    println!("  session  {}", style.cyan(&id));
    println!(
        "  repo     {}",
        paths::to_slashes(&root.display().to_string())
    );
    println!(
        "  agent    {}",
        settings.agent.as_deref().unwrap_or("unspecified")
    );
    println!("  state    {}", session::state_dir(root).display());
    println!();
    println!("Start the agent in another terminal. Before you send it a prompt, run:");
    println!("  flightstrip mark \"what you asked for\"");
    println!("and record what it runs with:");
    println!("  flightstrip exec -- cargo test");
    println!();

    let opts = WatchOptions {
        poll_ms: settings.interval_ms,
        baseline: settings.baseline,
        verbose: settings.verbose,
        extra_ignores: config.ignore_globs.clone(),
    };

    let recorded = {
        let mut recorder = Recorder::new(root, &mut log, opts);
        if settings.baseline {
            match recorder.seed_baseline() {
                Ok(count) => println!("Hashed {count} files as the starting state.\n"),
                Err(error) => eprintln!("warning: could not hash the repository: {error}"),
            }
        }
        println!("{} Ctrl+C to stop.\n", style.info("Recording."));
        recorder.run(stop)
    };

    // Close the session whatever happened above: a half-written log with no
    // `session_end` still exports, but a clean one is nicer to read.
    let close_result = log.close();
    // This is the cleanup that makes a crashed or interrupted recorder safe to
    // start again. It runs before the close error is reported.
    session::clear_active(root);
    if let Err(error) = recorded {
        eprintln!("warning: the recorder stopped early: {error}");
    }
    close_result.context("while closing the session")?;

    let events = log.events().unwrap_or_default();
    let changes = events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::FileChange(_)))
        .count();
    let commands = events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::Command(_)))
        .count();
    let tests = events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::TestRun(_)))
        .count();

    println!("\nSession closed: {id}");
    println!("  {changes} file changes, {commands} commands, {tests} test runs");
    println!("  export it with: flightstrip export {id} --out session.json");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn stopping_the_recorder_closes_the_session_and_clears_active() {
        let root = std::env::temp_dir().join(format!("fs-watch-stop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let stop = AtomicBool::new(false);
        stop.store(true, Ordering::SeqCst);
        record(
            &root,
            "stop test",
            &Settings {
                title: Some("stop test".into()),
                agent: None,
                interval_ms: 50,
                baseline: false,
                verbose: false,
            },
            &Config::default(),
            Style::plain(),
            &stop,
        )
        .unwrap();

        assert!(session::read_active(&root).is_none());
        let sessions = crate::cmd::all_sessions(&root);
        assert_eq!(sessions.len(), 1);
        let events = session::read_events(&sessions[0]).unwrap();
        assert!(matches!(events.last().unwrap().kind, EventKind::SessionEnd));
        let _ = std::fs::remove_dir_all(&root);
    }
}
