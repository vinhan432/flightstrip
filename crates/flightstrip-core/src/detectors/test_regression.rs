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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::test_util::{file_event, test_event};
    use crate::detectors::NoDiffs;
    use crate::schema::FileOp;

    fn run(events: &[Event]) -> Vec<Incident> {
        TestRegression.detect(&DetectInput::new(events, &NoDiffs))
    }

    #[test]
    fn green_then_red_is_reported_with_the_suspects() {
        let events = [
            test_event(0, 10, "cargo test", 3, 0, 0, true),
            file_event(1, 20, "src/math.rs", FileOp::Modify),
            test_event(2, 30, "cargo test", 2, 1, 0, false),
        ];
        let found = run(&events);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Critical);
        assert!(found[0].evidence.contains("cargo test"));
        assert!(found[0].evidence.contains("src/math.rs"));
    }

    #[test]
    fn red_without_a_green_run_is_not_a_regression() {
        let events = [test_event(0, 10, "cargo test", 0, 1, 0, false)];
        assert!(run(&events).is_empty());
    }

    #[test]
    fn repeated_red_runs_do_not_multiply_the_incident() {
        let events = [
            test_event(0, 10, "cargo test", 3, 0, 0, true),
            test_event(1, 20, "cargo test", 2, 1, 0, false),
            test_event(2, 30, "cargo test", 2, 1, 0, false),
        ];
        assert_eq!(run(&events).len(), 1);
    }

    #[test]
    fn a_later_green_run_does_not_erase_the_earlier_regression() {
        let events = [
            test_event(0, 10, "cargo test", 3, 0, 0, true),
            test_event(1, 20, "cargo test", 2, 1, 0, false),
            test_event(2, 30, "cargo test", 3, 0, 0, true),
        ];
        assert_eq!(run(&events).len(), 1);
    }
}
