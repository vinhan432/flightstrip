//! One module per subcommand.

pub mod completions;
pub mod doctor;
pub mod exec_cmd;
pub mod export_cmd;
pub mod init;
pub mod list;
pub mod mark;
pub mod prune;
pub mod report_cmd;
pub mod shell_init;
pub mod status;
pub mod verify;
pub mod watch;

use crate::error::Result;
use flightstrip_core::session::{self, ActiveRef};
use std::path::{Path, PathBuf};

/// The session an `exec`/`mark`/`export` should attach to: `"latest"` means the
/// most recently started recorded session.
pub fn resolve_session(root: &Path, wanted: &str) -> Result<String> {
    if wanted != "latest" {
        let dir = session::session_dir(root, wanted);
        if !dir.is_dir() {
            crate::bail!(
                "no session called {wanted} in {}",
                session::sessions_dir(root).display()
            );
        }
        return Ok(wanted.to_string());
    }
    latest_session(root).ok_or_else(|| {
        format!(
            "no sessions yet; record one with `flightstrip watch` (looked in {})",
            session::sessions_dir(root).display()
        )
    })
}

/// Session ids sort chronologically because they start with a UTC timestamp.
pub fn latest_session(root: &Path) -> Option<String> {
    let dir = session::sessions_dir(root);
    let mut ids: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    ids.sort();
    ids.pop()
}

/// Session dirs in chronological order.
pub fn all_sessions(root: &Path) -> Vec<PathBuf> {
    let dir = session::sessions_dir(root);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|read| {
            read.flatten()
                .filter(|entry| entry.path().is_dir())
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

/// Read `.flightstrip/active`, and clean up a pointer whose process is gone so a
/// crashed session cannot block the next one forever.
pub fn active_session(root: &Path) -> Result<String> {
    let active: ActiveRef = session::read_active(root).ok_or_else(|| {
        "no active session; start one with `flightstrip watch` in another terminal".to_string()
    })?;
    if !session::pid_alive(active.pid) {
        session::clear_active(root);
        crate::bail!(
            "the active session ({}) is no longer running; its pointer was cleared",
            active.id
        );
    }
    if !session::session_dir(root, &active.id).is_dir() {
        crate::bail!("active session {} has no directory", active.id);
    }
    Ok(active.id)
}
