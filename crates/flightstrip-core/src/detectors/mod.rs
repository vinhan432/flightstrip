//! Deterministic incident detectors.
//!
//! Every detector is a pure function of (event log, available diffs). No I/O, no
//! clock, no randomness — which is why the same session always produces the same
//! `session.json`, and why these are cheap to unit test.

pub mod dangerous_command;
pub mod dependency_added;
pub mod secret_touched;
pub mod test_regression;
pub mod test_weakened;
pub mod thrashing;

use crate::schema::{DiffHunk, Event, Incident, IncidentKind, Severity};

/// Read-only access to the hunks a detector needs. Export pre-computes them from
/// the snapshot store; tests can hand over a stub.
pub trait DiffSource {
    fn hunks_for(&self, seq: u64) -> &[DiffHunk];
}

/// Source used when no snapshots are available (e.g. a store that was pruned).
pub struct NoDiffs;

impl DiffSource for NoDiffs {
    fn hunks_for(&self, _seq: u64) -> &[DiffHunk] {
        &[]
    }
}

pub struct DetectInput<'a> {
    pub events: &'a [Event],
    pub diffs: &'a dyn DiffSource,
}

impl<'a> DetectInput<'a> {
    pub fn new(events: &'a [Event], diffs: &'a dyn DiffSource) -> Self {
        DetectInput { events, diffs }
    }
}

pub trait Detector {
    fn name(&self) -> &'static str;
    fn detect(&self, input: &DetectInput) -> Vec<Incident>;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn incident(
    step: u64,
    t_ms: u64,
    kind: IncidentKind,
    severity: Severity,
    title: impl Into<String>,
    evidence: impl Into<String>,
    why: impl Into<String>,
    prompt_id: Option<String>,
    path: Option<String>,
) -> Incident {
    Incident {
        id: String::new(),
        kind,
        severity,
        step,
        t_ms,
        title: title.into(),
        evidence: evidence.into(),
        why_it_matters: why.into(),
        prompt_id,
        path,
    }
}

/// Evidence is a table cell in the report and a paragraph in the drawer, so it
/// gets flattened and capped rather than being allowed to run for pages.
pub(crate) fn compose_evidence(lines: &[String], cap: usize) -> String {
    let joined = lines
        .iter()
        .take(3)
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    if joined.chars().count() > cap {
        let mut out: String = joined.chars().take(cap.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        joined
    }
}

/// Run every registered detector, then order and id the incidents. Ordering is
/// by step so the strip chart can place markers without sorting again.
pub fn run_all(input: &DetectInput) -> Vec<Incident> {
    let registry: [&dyn Detector; 6] = [
        &test_weakened::TestWeakened,
        &test_regression::TestRegression,
        &thrashing::Thrashing,
        &dangerous_command::DangerousCommand,
        &dependency_added::DependencyAdded,
        &secret_touched::SecretTouched,
    ];

    let mut all: Vec<Incident> = Vec::new();
    for detector in registry {
        all.extend(detector.detect(input));
    }
    all.sort_by(|a, b| a.step.cmp(&b.step).then(a.severity.cmp(&b.severity)));
    for (index, item) in all.iter_mut().enumerate() {
        item.id = format!("i-{index}");
    }
    all
}
