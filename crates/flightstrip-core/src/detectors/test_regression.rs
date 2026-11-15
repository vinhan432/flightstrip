//! A green suite that turned red, with the changes that happened in between.

use super::{incident, DetectInput, Detector};
use crate::schema::{Event, EventKind, Incident, IncidentKind, Severity};

pub struct TestRegression;

impl Detector for TestRegression {
    fn name(&self) -> &'static str {
        "test_regression"
    }

    fn detect(&self, input: &DetectInput) -> Vec<Incident> {
        let mut found = Vec::new();
        let mut last_ok: Option<(bool, &Event)> = None;

        for event in input.events.iter() {
            let EventKind::TestRun(run) = &event.kind else {
                continue;
            };

            if let Some((true, previous)) = last_ok {
                if !run.ok {
                    // Everything touched between the last green run and this red
                    // one is the suspect list.
                    let between: Vec<String> = input
                        .events
                        .iter()
                        .filter(|e| e.seq > previous.seq && e.seq < event.seq)
                        .filter_map(|e| match &e.kind {
                            EventKind::FileChange(fc) => Some(fc.path.clone()),
                            _ => None,
                        })
                        .collect();

                    let mut evidence = format!(
                        "{} · {} passed / {} failed",
                        run.cmd.trim(),
                        run.passed,
                        run.failed
                    );
                    if !between.is_empty() {
                        evidence.push_str(" · touched: ");
                        evidence.push_str(&between.join(", "));
                    }

                    found.push(incident(
                        event.seq,
                        event.t_ms,
                        IncidentKind::TestRegression,
                        Severity::Critical,
                        "Tests went from passing to failing".to_string(),
                        evidence,
                        "This is the signal the whole recording exists for: the suite was green, \
                         the agent kept working, and it is red now.",
                        run.prompt_id.clone(),
                        between.first().cloned(),
                    ));
                }
            }

            last_ok = Some((run.ok, event));
        }

        found
    }
}
