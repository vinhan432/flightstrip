//! `flightstrip export`: event log → `session.json`.

use crate::cmd;
use crate::error::{Context, Result};
use flightstrip_core::differ::DiffOptions;
use flightstrip_core::export::{self, ExportOptions};
use flightstrip_core::schema::{SessionLabel, Severity};
use flightstrip_core::session;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub fn run(
    root: &Path,
    wanted: &str,
    out: Option<PathBuf>,
    redact: bool,
    attribution_gap_ms: u64,
    fixture: bool,
    no_hunks: bool,
    max_hunk_lines: Option<usize>,
    requested_label: Option<SessionLabel>,
) -> Result<()> {
    let id = cmd::resolve_session(root, wanted)?;
    let dir = session::session_dir(root, &id);
    let label = requested_label.unwrap_or(if fixture {
        SessionLabel::Fixture
    } else {
        session::read_record(&dir)
            .map(|r| r.label)
            .unwrap_or(SessionLabel::Recorded)
    });
    let mut diff = DiffOptions::default();
    if no_hunks {
        diff.max_hunks = 0;
    }
    if let Some(lines) = max_hunk_lines {
        diff.max_lines_per_hunk = lines;
    }
    let opts = ExportOptions {
        label,
        redact,
        attribution_gap_ms,
        diff,
    };

    let session_export = export::export(root, &dir, &opts)
        .with_context(|| format!("while exporting session {id}"))?;

    let out = out.unwrap_or_else(|| dir.join("session.json"));
    export::write_session_json(&out, &session_export)
        .with_context(|| format!("while writing {}", out.display()))?;

    let critical = count(&session_export.incidents, Severity::Critical);
    let warn = count(&session_export.incidents, Severity::Warn);
    let info = count(&session_export.incidents, Severity::Info);

    let size = std::fs::metadata(&out).map(|meta| meta.len()).unwrap_or(0);
    println!("Wrote {} ({})", out.display(), human_bytes(size));
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
    if no_hunks {
        println!("  diff hunks were omitted");
    }
    Ok(())
}

fn count(incidents: &[flightstrip_core::schema::Incident], severity: Severity) -> usize {
    incidents.iter().filter(|i| i.severity == severity).count()
}

fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}
