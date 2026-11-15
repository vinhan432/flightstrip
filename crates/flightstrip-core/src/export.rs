//! Event log → `session.json`.
//!
//! This is the only module that assembles the export, and it is deliberately
//! boring: validate, attribute, diff, detect, summarise, redact, write.

use crate::attributor::{self, DEFAULT_GAP_MS};
use crate::detectors::{run_all, DetectInput, DiffSource};
use crate::differ::{self, DiffOptions};
use crate::redact;
use crate::schema::*;
use crate::session::{self, read_record};
use crate::store::SnapshotStore;
use crate::time;
use crate::{io_error, json, paths, Error, Result};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub label: SessionLabel,
    pub redact: bool,
    pub attribution_gap_ms: u64,
    pub diff: DiffOptions,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            label: SessionLabel::Recorded,
            redact: false,
            attribution_gap_ms: DEFAULT_GAP_MS,
            diff: DiffOptions::default(),
        }
    }
}

struct PrecomputedDiffs {
    by_seq: HashMap<u64, Vec<DiffHunk>>,
}

impl DiffSource for PrecomputedDiffs {
    fn hunks_for(&self, seq: u64) -> &[DiffHunk] {
        self.by_seq.get(&seq).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// Content for one side of a change. `None` means "the object is missing", which
/// is different from "this side did not exist".
fn side(store: &SnapshotStore, hash: &Option<String>) -> Option<Vec<u8>> {
    match hash {
        None => Some(Vec::new()),
        Some(h) => store.get(h).ok(),
    }
}

pub fn export(root: &Path, dir: &Path, opts: &ExportOptions) -> Result<SessionExport> {
    let mut events = session::read_events(dir)?;
    if events.is_empty() {
        return Err(Error::Invalid(
            "the session has no events (was it ever started?)".into(),
        ));
    }
    if !matches!(events[0].kind, EventKind::SessionStart(_)) {
        return Err(Error::Invalid(
            "the first event must be session_start".into(),
        ));
    }

    // A session interrupted by a crash still has to export.
    let last_t = events.last().map(|e| e.t_ms).unwrap_or(0);
    let last_seq = events.last().map(|e| e.seq).unwrap_or(0);
    if !matches!(events.last().map(|e| &e.kind), Some(EventKind::SessionEnd)) {
        events.push(Event::new(last_seq + 1, last_t, EventKind::SessionEnd));
    }

    attributor::attribute(&mut events, opts.attribution_gap_ms);

    // Diffs, computed once and shared with the detectors.
    let store = SnapshotStore::at(root);
    let mut by_seq: HashMap<u64, Vec<DiffHunk>> = HashMap::new();
    for event in &events {
        let EventKind::FileChange(change) = &event.kind else {
            continue;
        };
        if change.before_hash.is_none() && change.after_hash.is_none() {
            continue;
        }
        let (Some(old), Some(new)) = (
            side(&store, &change.before_hash),
            side(&store, &change.after_hash),
        ) else {
            // Store was pruned or the object is corrupt: no hunks rather than a
            // wrong full-file rewrite.
            continue;
        };
        by_seq.insert(event.seq, differ::hunks(&old, &new, &opts.diff));
    }
    let diffs = PrecomputedDiffs { by_seq };

    let incidents = run_all(&DetectInput::new(&events, &diffs));

    let record = read_record(dir);
    let id = record.as_ref().map(|r| r.id.clone()).unwrap_or_else(|| {
        dir.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "session".into())
    });
    let started_at = record
        .as_ref()
        .map(|r| r.started_at.clone())
        .or_else(|| session::parse_started_at(&id).map(time::format_rfc3339))
        .unwrap_or_else(|| "1970-01-01T00:00:00.000Z".to_string());

    let meta = SessionMeta {
        id,
        title: record
            .as_ref()
            .map(|r| r.title.clone())
            .unwrap_or_else(|| "Untitled session".into()),
        agent: record.as_ref().and_then(|r| r.agent.clone()),
        started_at,
        duration_ms: last_t,
        os: record
            .as_ref()
            .map(|r| r.os.clone())
            .unwrap_or_else(|| paths::os_name().into()),
        repo_name: record
            .as_ref()
            .map(|r| r.repo_name.clone())
            .unwrap_or_else(|| "repository".into()),
        label: opts.label,
    };

    let mut prompts = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let EventKind::Prompt(prompt) = &event.kind else {
            continue;
        };
        prompts.push(PromptSummary {
            id: prompt.id.clone(),
            text: prompt.text.clone(),
            step: index as u64,
            t_ms: event.t_ms,
            files_touched: events
                .iter()
                .filter(|e| {
                    matches!(&e.kind, EventKind::FileChange(fc) if fc.prompt_id.as_deref() == Some(prompt.id.as_str()))
                })
                .count() as u32,
            commands_run: events
                .iter()
                .filter(|e| {
                    matches!(&e.kind, EventKind::Command(c) if c.prompt_id.as_deref() == Some(prompt.id.as_str()))
                })
                .count() as u32,
            incidents: incidents
                .iter()
                .filter(|i| i.prompt_id.as_deref() == Some(prompt.id.as_str()))
                .count() as u32,
        });
    }

    let mut files: BTreeMap<String, FileSummary> = BTreeMap::new();
    for event in &events {
        let EventKind::FileChange(change) = &event.kind else {
            continue;
        };
        let entry = files.entry(change.path.clone()).or_default();
        entry.churn += 1;
        *entry.ops.entry(change.op.as_str().to_string()).or_insert(0) += 1;
        if let Some(prompt) = &change.prompt_id {
            if !entry.prompts.contains(prompt) {
                entry.prompts.push(prompt.clone());
            }
        }
    }

    let steps: Vec<ExportStep> = events
        .iter()
        .map(|event| ExportStep {
            seq: event.seq,
            t_ms: event.t_ms,
            kind: event.kind.clone(),
            hunks: diffs.by_seq.get(&event.seq).cloned(),
        })
        .collect();

    let mut incidents_by_kind: BTreeMap<String, u32> = BTreeMap::new();
    for incident in &incidents {
        *incidents_by_kind
            .entry(incident.kind.as_str().to_string())
            .or_insert(0) += 1;
    }
    let test_steps: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::TestRun(_)))
        .collect();
    let tests_first_ok = test_steps.first().map(|e| match &e.kind {
        EventKind::TestRun(t) => t.ok,
        _ => false,
    });
    let tests_last_ok = test_steps.last().map(|e| match &e.kind {
        EventKind::TestRun(t) => t.ok,
        _ => false,
    });

    let stats = SessionStats {
        steps: steps.len() as u64,
        files_touched: files.len() as u32,
        commands: events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Command(_)))
            .count() as u32,
        test_runs: test_steps.len() as u32,
        incidents_by_kind,
        tests_first_ok,
        tests_last_ok,
    };

    let mut export = SessionExport {
        version: SCHEMA_VERSION,
        meta,
        prompts,
        steps,
        incidents,
        files,
        stats,
    };

    if opts.redact {
        redact::redact_export(&mut export);
    }
    Ok(export)
}

pub fn write_session_json(path: &Path, session: &SessionExport) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        }
    }
    let text = json::to_pretty(&session.to_json()) + "\n";
    fs::write(path, text).map_err(|e| io_error(path, e))
}
