//! `flightstrip list`: every recorded session in this repository.

use crate::cmd;
use crate::config::Config;
use crate::error::Result;
use crate::style::{self, Style};
use flightstrip_core::export::{self, ExportOptions};
use flightstrip_core::json;
use flightstrip_core::schema::{EventKind, SessionExport, SessionLabel, Severity};
use flightstrip_core::session;
use flightstrip_core::time;
use std::path::Path;

struct Row {
    id: String,
    title: String,
    agent: Option<String>,
    started_at: String,
    duration_ms: u64,
    events: usize,
    files: usize,
    tests: usize,
    prompts: usize,
    critical: usize,
    warn: usize,
    info: usize,
    label: SessionLabel,
    active: bool,
    export_error: Option<String>,
}

pub fn run(root: &Path, config: &Config, as_json: bool, quiet: bool, style: Style) -> Result<()> {
    let active = session::read_active(root).filter(|a| session::pid_alive(a.pid));
    let dirs = cmd::all_sessions(root);
    if dirs.is_empty() {
        if as_json {
            style::emit("{\"sessions\":[],\"active\":null}");
        } else if quiet {
            return Ok(());
        } else {
            println!("No sessions recorded in this repository yet.");
            println!("Start one with: flightstrip init && flightstrip watch");
        }
        return Ok(());
    }

    let rows: Vec<Row> = dirs
        .iter()
        .map(|dir| {
            let record = session::read_record(dir);
            let events = session::read_events(dir).unwrap_or_default();
            let id = dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let exported = export::export(
                root,
                dir,
                &ExportOptions {
                    label: record
                        .as_ref()
                        .map(|r| r.label)
                        .unwrap_or(SessionLabel::Recorded),
                    attribution_gap_ms: config.attribution_decay_minutes.saturating_mul(60_000),
                    thrash_threshold: config.thrash_threshold,
                    ..ExportOptions::default()
                },
            );
            let (session_export, export_error): (Option<SessionExport>, Option<String>) =
                match exported {
                    Ok(export) => (Some(export), None),
                    Err(error) => (None, Some(error.to_string())),
                };
            let critical = session_export
                .as_ref()
                .map(|s| count(&s.incidents, Severity::Critical))
                .unwrap_or(0);
            let warn = session_export
                .as_ref()
                .map(|s| count(&s.incidents, Severity::Warn))
                .unwrap_or(0);
            let info = session_export
                .as_ref()
                .map(|s| count(&s.incidents, Severity::Info))
                .unwrap_or(0);
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
            let prompts = events
                .iter()
                .filter(|e| matches!(e.kind, EventKind::Prompt(_)))
                .count();
            let duration_ms = events.last().map(|e| e.t_ms).unwrap_or(0);
            Row {
                id: id.clone(),
                title: record
                    .as_ref()
                    .map(|r| r.title.clone())
                    .unwrap_or_else(|| "?".into()),
                agent: record.as_ref().and_then(|r| r.agent.clone()),
                started_at: record
                    .as_ref()
                    .map(|r| r.started_at.clone())
                    .unwrap_or_else(|| "?".into()),
                duration_ms,
                events: events.len(),
                files,
                tests,
                prompts,
                critical,
                warn,
                info,
                label: record
                    .as_ref()
                    .map(|r| r.label)
                    .unwrap_or(SessionLabel::Recorded),
                active: active.as_ref().map(|a| a.id == id).unwrap_or(false),
                export_error,
            }
        })
        .collect();

    if as_json {
        let sessions = JsonRows(&rows).to_json();
        style::emit(&json::to_pretty(&json::obj(vec![
            ("sessions", sessions),
            (
                "active",
                match &active {
                    Some(active) => json::Json::from(&active.id),
                    None => json::Json::Null,
                },
            ),
        ])));
        return Ok(());
    }

    if quiet {
        for row in &rows {
            let marker = if row.active { " *" } else { "" };
            println!("{}{marker}", row.id);
        }
        return Ok(());
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{:<26} {:<19} {:>9} {:>7} {:>11}  TITLE\n",
        style.bold("SESSION"),
        style.bold("STARTED (UTC)"),
        style.bold("DURATION"),
        style.bold("EVENTS"),
        style.bold("INCIDENTS")
    ));
    for row in &rows {
        let started = row.started_at.replace('T', " ");
        let started = started.chars().take(19).collect::<String>();
        let incidents = if row.export_error.is_some() {
            "?".to_string()
        } else if row.critical > 0 {
            style.red(&format!("{}c", row.critical))
        } else if row.warn > 0 {
            style.yellow(&format!("{}w", row.warn))
        } else if row.info > 0 {
            format!("{}i", row.info)
        } else {
            "-".to_string()
        };
        let mut title = truncate(&row.title, 48);
        if row.label == SessionLabel::Fixture {
            title.push_str(" [fixture]");
        }
        if row.active {
            title.push_str("  *");
        }
        out.push_str(&format!(
            "{:<26} {:<19} {:>9} {:>7} {:>11}  {}\n",
            row.id,
            started,
            time::human_duration(row.duration_ms),
            row.events,
            incidents,
            title
        ));
    }
    out.push_str(&format!(
        "\n{} sessions · {} files · {} test runs · {} prompts",
        rows.len(),
        rows.iter().map(|r| r.files).sum::<usize>(),
        rows.iter().map(|r| r.tests).sum::<usize>(),
        rows.iter().map(|r| r.prompts).sum::<usize>()
    ));
    if let Some(active) = active {
        out.push_str(&format!("\nRecording now: {}", style.cyan(&active.id)));
    }
    if rows.iter().any(|r| r.export_error.is_some()) {
        out.push_str("\nSome sessions could not be exported; run `flightstrip doctor`.");
    }
    style::emit(&out);
    Ok(())
}

fn count(incidents: &[flightstrip_core::schema::Incident], severity: Severity) -> usize {
    incidents.iter().filter(|i| i.severity == severity).count()
}

fn truncate(text: &str, max: usize) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max - 1).collect::<String>())
    }
}

struct JsonRows<'a>(&'a [Row]);

impl JsonRows<'_> {
    fn to_json(&self) -> json::Json {
        json::Json::Arr(self.0.iter().map(Row::to_json).collect())
    }
}

impl Row {
    fn to_json(&self) -> json::Json {
        json::obj(vec![
            ("id", json::Json::from(&self.id)),
            ("title", json::Json::from(&self.title)),
            ("agent", json::Json::from(self.agent.clone())),
            ("started_at", json::Json::from(&self.started_at)),
            ("duration_ms", json::Json::from(self.duration_ms)),
            ("events", json::Json::from(self.events)),
            ("files", json::Json::from(self.files)),
            ("tests", json::Json::from(self.tests)),
            ("prompts", json::Json::from(self.prompts)),
            (
                "incidents",
                json::obj(vec![
                    ("critical", json::Json::from(self.critical)),
                    ("warn", json::Json::from(self.warn)),
                    ("info", json::Json::from(self.info)),
                ]),
            ),
            ("label", json::Json::from(self.label.as_str())),
            ("active", json::Json::from(self.active)),
            (
                "export_error",
                match &self.export_error {
                    Some(error) => json::Json::from(error),
                    None => json::Json::Null,
                },
            ),
        ])
    }
}
