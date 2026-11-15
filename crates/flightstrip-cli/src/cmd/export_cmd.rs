//! `flightstrip export`: event log → `session.json`.

use crate::cmd;
use crate::error::{Context, Result};
use flightstrip_core::export::{self, ExportOptions};
use flightstrip_core::schema::{SessionLabel, Severity};
use flightstrip_core::session;
use std::path::{Path, PathBuf};

pub fn run(
    root: &Path,
    wanted: &str,
    out: Option<PathBuf>,
    redact: bool,
    attribution_gap_ms: u64,
    fixture: bool,
) -> Result<()> {
    let id = cmd::resolve_session(root, wanted)?;
    let dir = session::session_dir(root, &id);
    let label = if fixture {
        SessionLabel::Fixture
    } else {
        session::read_record(&dir)
            .map(|r| r.label)
            .unwrap_or(SessionLabel::Recorded)
    };
    let opts = ExportOptions {
        label,
        redact,
        attribution_gap_ms,
        ..ExportOptions::default()
    };

    let session_export = export::export(root, &dir, &opts)
        .with_context(|| format!("while exporting session {id}"))?;

    let out = out.unwrap_or_else(|| dir.join("session.json"));
    export::write_session_json(&out, &session_export)
        .with_context(|| format!("while writing {}", out.display()))?;

    let critical = count(&session_export.incidents, Severity::Critical);
    let warn = count(&session_export.incidents, Severity::Warn);
    let info = count(&session_export.incidents, Severity::Info);

    println!("Wrote {}", out.display());
    println!(
        "  {} steps · {} prompts · {} files · {} incidents ({critical} critical, {warn} warn, {info} info)",
        session_export.stats.steps,
        session_export.prompts.len(),
        session_export.stats.files_touched,
        session_export.incidents.len()
    );
    if redact {
        println!("  secret-looking values were redacted");
    }
    Ok(())
}

fn count(incidents: &[flightstrip_core::schema::Incident], severity: Severity) -> usize {
    incidents.iter().filter(|i| i.severity == severity).count()
}
