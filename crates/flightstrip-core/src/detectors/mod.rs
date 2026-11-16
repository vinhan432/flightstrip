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
use std::collections::HashMap;

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

/// Run every registered detector, then order and id the incidents.
///
/// Ordering is `(step, t_ms, kind)`, not detector registration order: two
/// receivers can see the same event (a removed assertion in a file that also
/// contains a literal credential), and the output has to be byte-identical on
/// every run. Ids are `kind-NNNN`, numbered per kind in that order, so a
/// reviewer can cite `test_weakened-0001` in a PR comment without it moving
/// when an unrelated detector gains a rule.
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
    all.sort_by(|a, b| {
        a.step
            .cmp(&b.step)
            .then(a.t_ms.cmp(&b.t_ms))
            .then(a.kind.as_str().cmp(b.kind.as_str()))
    });

    let mut counters: HashMap<&'static str, u32> = HashMap::new();
    for item in all.iter_mut() {
        let count = counters.entry(item.kind.as_str()).or_insert(0);
        *count += 1;
        item.id = format!("{}-{:04}", item.kind.as_str(), count);
    }
    all
}

#[cfg(test)]
pub(crate) mod test_util {
    use super::DiffSource;
    use crate::schema::{
        Command, CommandSource, DiffHunk, DiffLine, DiffTag, Event, EventKind, FileChange, FileOp,
        TestRun,
    };
    use std::collections::HashMap;

    /// A `DiffSource` built from `(seq, lines)` pairs, only for tests.
    pub struct StubDiffs {
        by_seq: HashMap<u64, Vec<DiffHunk>>,
    }

    impl StubDiffs {
        pub fn new() -> Self {
            StubDiffs {
                by_seq: HashMap::new(),
            }
        }

        pub fn with(mut self, seq: u64, lines: &[(DiffTag, &str)]) -> Self {
            let lines = lines
                .iter()
                .map(|(tag, text)| DiffLine {
                    tag: *tag,
                    text: (*text).to_string(),
                })
                .collect();
            self.by_seq.insert(
                seq,
                vec![DiffHunk {
                    old_start: 1,
                    new_start: 1,
                    lines,
                }],
            );
            self
        }
    }

    impl DiffSource for StubDiffs {
        fn hunks_for(&self, seq: u64) -> &[DiffHunk] {
            self.by_seq.get(&seq).map(Vec::as_slice).unwrap_or(&[])
        }
    }

    pub fn file_event(seq: u64, t_ms: u64, path: &str, op: FileOp) -> Event {
        Event::new(
            seq,
            t_ms,
            EventKind::FileChange(FileChange {
                path: path.to_string(),
                op,
                before_hash: None,
                after_hash: None,
                lines_added: 0,
                lines_removed: 0,
                prompt_id: None,
            }),
        )
    }

    pub fn command_event(seq: u64, t_ms: u64, cmd: &str, exit_code: i32) -> Event {
        Event::new(
            seq,
            t_ms,
            EventKind::Command(Command {
                cmd: cmd.to_string(),
                exit_code: Some(exit_code),
                duration_ms: Some(1),
                source: CommandSource::Exec,
                prompt_id: None,
            }),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn test_event(
        seq: u64,
        t_ms: u64,
        cmd: &str,
        passed: u32,
        failed: u32,
        skipped: u32,
        ok: bool,
    ) -> Event {
        Event::new(
            seq,
            t_ms,
            EventKind::TestRun(TestRun {
                cmd: cmd.to_string(),
                passed,
                failed,
                skipped,
                total: passed + failed + skipped,
                ok,
                prompt_id: None,
            }),
        )
    }

    #[test]
    fn incidents_are_stably_sorted_and_identified() {
        use super::{run_all, DetectInput};

        let events = [
            command_event(0, 500, "rm -rf /", 0),
            command_event(1, 100, "git push --force origin main", 0),
        ];
        let diffs = StubDiffs::new();
        let first = run_all(&DetectInput::new(&events, &diffs));
        let second = run_all(&DetectInput::new(&events, &diffs));

        assert_eq!(first, second, "the same events must produce the same ids");
        assert_eq!(
            first.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["dangerous_command-0001", "dangerous_command-0002"]
        );
        assert_eq!(
            first.iter().map(|i| i.step).collect::<Vec<_>>(),
            vec![0, 1],
            "step order is the primary sort key"
        );
    }
}
