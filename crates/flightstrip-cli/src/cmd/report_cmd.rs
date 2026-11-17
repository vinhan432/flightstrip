//! `flightstrip report`: `session.json` → the markdown trust summary.

use crate::cmd;
use crate::config::Config;
use crate::error::{Context, Result};
use crate::style::Style;
use flightstrip_core::export::{self, ExportOptions};
use flightstrip_core::report;
use flightstrip_core::schema::{SessionExport, Severity};
use flightstrip_core::session;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Github,
    Json,
}

/// Returns `true` when the incident severity met `--fail-on`, so the caller can
/// exit non-zero without printing an error message for a policy failure.
#[allow(clippy::too_many_arguments)]
pub fn run(
    root: &Path,
    wanted: &str,
    out: Option<PathBuf>,
    redact: bool,
    format: Format,
    fail_on: Option<Severity>,
    config: &Config,
    style: Style,
) -> Result<bool> {
    let session_export = load(root, wanted, redact, config)?;
    let body = match format {
        Format::Markdown => report::render(&session_export),
        Format::Github => report::render_github(&session_export),
        Format::Json => session_export.to_pretty_string() + "\n",
    };

    print!("{body}");
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("while creating {}", parent.display()))?;
            }
        }
        std::fs::write(&path, &body)
            .with_context(|| format!("while writing {}", path.display()))?;
        eprintln!(
            "\n{} report written to {}",
            style.pass("ok:"),
            path.display()
        );
    }

    let gated = fail_on
        .map(|threshold| report::meets_threshold(&session_export, threshold))
        .unwrap_or(false);
    Ok(gated)
}

/// A `.json` argument is read directly; anything else is treated as a session id
/// and exported fresh, so `report` never shows a stale file.
fn load(root: &Path, wanted: &str, redact: bool, config: &Config) -> Result<SessionExport> {
    if wanted.to_ascii_lowercase().ends_with(".json") {
        let path = PathBuf::from(wanted);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("while reading {}", path.display()))?;
        let mut export = SessionExport::parse(&text)
            .with_context(|| format!("{} is not a Flightstrip session", path.display()))?;
        if redact {
            flightstrip_core::redact::redact_export(&mut export);
        }
        return Ok(export);
    }

    let id = cmd::resolve_session(root, wanted)?;
    let dir = session::session_dir(root, &id);
    let opts = ExportOptions {
        label: session::read_record(&dir)
            .map(|r| r.label)
            .unwrap_or(flightstrip_core::schema::SessionLabel::Recorded),
        redact,
        attribution_gap_ms: config.attribution_decay_minutes.saturating_mul(60_000),
        thrash_threshold: config.thrash_threshold,
        ..ExportOptions::default()
    };
    export::export(root, &dir, &opts).with_context(|| format!("while exporting session {id}"))
}
