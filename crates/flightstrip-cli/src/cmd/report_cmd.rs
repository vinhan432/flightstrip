//! `flightstrip report`: `session.json` → the markdown trust summary.

use crate::cmd;
use crate::error::{Context, Result};
use flightstrip_core::export::{self, ExportOptions};
use flightstrip_core::report;
use flightstrip_core::schema::SessionExport;
use flightstrip_core::session;
use std::path::{Path, PathBuf};

pub fn run(root: &Path, wanted: &str, out: Option<PathBuf>, redact: bool) -> Result<()> {
    let session_export = load(root, wanted, redact)?;
    let markdown = report::render(&session_export);

    print!("{markdown}");
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("while creating {}", parent.display()))?;
            }
        }
        std::fs::write(&path, &markdown)
            .with_context(|| format!("while writing {}", path.display()))?;
        eprintln!("\nflightstrip: report written to {}", path.display());
    }
    Ok(())
}

/// A `.json` argument is read directly; anything else is treated as a session id
/// and exported fresh, so `report` never shows a stale file.
fn load(root: &Path, wanted: &str, redact: bool) -> Result<SessionExport> {
    if wanted.to_ascii_lowercase().ends_with(".json") {
        let path = PathBuf::from(wanted);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("while reading {}", path.display()))?;
        return SessionExport::parse(&text)
            .with_context(|| format!("{} is not a Flightstrip session", path.display()));
    }

    let id = cmd::resolve_session(root, wanted)?;
    let dir = session::session_dir(root, &id);
    let opts = ExportOptions {
        label: session::read_record(&dir)
            .map(|r| r.label)
            .unwrap_or(flightstrip_core::schema::SessionLabel::Recorded),
        redact,
        ..ExportOptions::default()
    };
    export::export(root, &dir, &opts).with_context(|| format!("while exporting session {id}"))
}
