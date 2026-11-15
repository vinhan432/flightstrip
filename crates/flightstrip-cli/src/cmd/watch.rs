//! `flightstrip watch`: the recording loop.

use crate::error::{Context, Result};
use crate::interrupt;
use flightstrip_core::paths;
use flightstrip_core::scan::{Recorder, WatchOptions};
use flightstrip_core::schema::{EventKind, SessionLabel};
use flightstrip_core::session::{self, EventLog};
use std::path::Path;

pub fn run(
    root: &Path,
    title: Option<String>,
    agent: Option<String>,
    interval_ms: u64,
    baseline: bool,
    verbose: bool,
) -> Result<()> {
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

    let title = title.unwrap_or_else(|| {
        let root_name = session::repo_name(root);
        format!("{root_name} session")
    });

    let mut log = EventLog::create(root, &title, agent.clone(), SessionLabel::Recorded)
        .context("while starting the session")?;
    let id = log.id();
    session::write_active(root, &id).context("while claiming the active session")?;
    interrupt::install().context("while installing the Ctrl+C handler")?;

    println!("Flightstrip recorder");
    println!("  session  {id}");
    println!(
        "  repo     {}",
        paths::to_slashes(&root.display().to_string())
    );
    println!("  agent    {}", agent.as_deref().unwrap_or("unspecified"));
    println!("  state    {}", session::state_dir(root).display());
    println!();
    println!("Start the agent in another terminal. Before you send it a prompt, run:");
    println!("  flightstrip mark \"what you asked for\"");
    println!("and record what it runs with:");
    println!("  flightstrip exec -- cargo test");
    println!();

    let opts = WatchOptions {
        poll_ms: interval_ms,
        baseline,
        verbose,
    };

    let recorded = {
        let mut recorder = Recorder::new(root, &mut log, opts);
        if baseline {
            match recorder.seed_baseline() {
                Ok(count) => println!("Hashed {count} files as the starting state.\n"),
                Err(error) => eprintln!("warning: could not hash the repository: {error}"),
            }
        }
        println!("Recording. Ctrl+C to stop.\n");
        recorder.run(interrupt::flag())
    };

    // Close the session whatever happened above: a half-written log with no
    // `session_end` still exports, but a clean one is nicer to read.
    let close_result = log.close();
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

    // A second Ctrl+C during the summary is not interesting.
    Ok(())
}
