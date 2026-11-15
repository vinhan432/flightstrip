//! `flightstrip list`: every recorded session in this repository.

use crate::cmd;
use crate::error::Result;
use flightstrip_core::schema::{EventKind, SessionLabel};
use flightstrip_core::session;
use std::path::Path;

pub fn run(root: &Path) -> Result<()> {
    let dirs = cmd::all_sessions(root);
    if dirs.is_empty() {
        println!("No sessions recorded in this repository yet.");
        println!("Start one with: flightstrip watch");
        return Ok(());
    }

    println!(
        "{:<26} {:<28} {:>7} {:>7} {:>6}  TITLE",
        "SESSION", "STARTED (UTC)", "EVENTS", "FILES", "TESTS"
    );
    for dir in dirs {
        let record = session::read_record(&dir);
        let events = session::read_events(&dir).unwrap_or_default();
        let id = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let started = record
            .as_ref()
            .map(|r| r.started_at.chars().take(19).collect::<String>())
            .unwrap_or_else(|| "?".into());
        let title = record
            .as_ref()
            .map(|r| r.title.clone())
            .unwrap_or_else(|| "?".into());
        let label = record
            .as_ref()
            .map(|r| match r.label {
                SessionLabel::Recorded => "",
                SessionLabel::Fixture => " [fixture]",
            })
            .unwrap_or("");
        let files = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::FileChange(fc) => Some(fc.path.as_str()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let tests = events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::TestRun(_)))
            .count();
        println!(
            "{:<26} {:<28} {:>7} {:>7} {:>6}  {}{}",
            id,
            started.replace('T', " "),
            events.len(),
            files,
            tests,
            truncate(&title, 48),
            label
        );
    }

    let active = session::read_active(root);
    if let Some(active) = active.filter(|a| session::pid_alive(a.pid)) {
        println!("\nRecording now: {}", active.id);
    }
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' { ' ' } else { c })
        .collect();
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max - 1).collect::<String>())
    }
}
