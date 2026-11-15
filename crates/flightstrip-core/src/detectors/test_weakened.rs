//! Tests that got weaker rather than fixed: assertions removed, skips added,
//! test files deleted, or a run that reports fewer tests than the one before it.
//!
//! The markers below are substrings rather than regexes. They are checked
//! case-insensitively against single diff lines, which is all the precision the
//! question needs: "does this removed line look like an assertion?"

use super::{compose_evidence, incident, DetectInput, Detector};
use crate::paths::is_test_path;
use crate::schema::{DiffTag, EventKind, FileOp, Incident, IncidentKind, Severity};
use std::collections::BTreeMap;

/// Lower-case markers that make a line an assertion.
const ASSERTION_MARKERS: &[&str] = &[
    "assert",
    "expect(",
    ".should",
    "assertequal",
    "assert_eq!",
    "asserttrue",
    "assertfalse",
    "check(",
    "require(",
];

/// Lower-case markers that make an added line a skip or a todo.
const SKIP_MARKERS: &[&str] = &[
    ".skip",
    ".todo",
    "xit(",
    "xdescribe(",
    "xtest(",
    "@pytest.mark.skip",
    "@unittest.skip",
    "#[ignore]",
    "t.skip(",
    "@disabled",
    "@ignore",
];

fn is_assertion(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    ASSERTION_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

fn is_skip(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    SKIP_MARKERS.iter().any(|marker| lower.contains(marker))
}

pub struct TestWeakened;

impl Detector for TestWeakened {
    fn name(&self) -> &'static str {
        "test_weakened"
    }

    fn detect(&self, input: &DetectInput) -> Vec<Incident> {
        let mut found = Vec::new();

        for (step, event) in input.events.iter().enumerate() {
            let EventKind::FileChange(change) = &event.kind else {
                continue;
            };
            if !is_test_path(&change.path) {
                continue;
            }
            let step = step as u64;

            if change.op == FileOp::Delete {
                found.push(incident(
                    step,
                    event.t_ms,
                    IncidentKind::TestWeakened,
                    Severity::Critical,
                    "Test file deleted",
                    change.path.clone(),
                    "Removing tests makes a failing assertion disappear instead of making it pass. \
                     A reviewer cannot tell a fix from a cover-up.",
                    change.prompt_id.clone(),
                    Some(change.path.clone()),
                ));
                continue;
            }

            let hunks = input.diffs.hunks_for(event.seq);
            if hunks.is_empty() {
                continue;
            }

            let removed: Vec<String> = hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .filter(|line| line.tag == DiffTag::Remove && is_assertion(&line.text))
                .map(|line| line.text.clone())
                .collect();
            if !removed.is_empty() {
                found.push(incident(
                    step,
                    event.t_ms,
                    IncidentKind::TestWeakened,
                    Severity::Critical,
                    format!(
                        "{} assertion{} removed",
                        removed.len(),
                        if removed.len() == 1 { "" } else { "s" }
                    ),
                    compose_evidence(&removed, 240),
                    "An assertion was deleted or loosened in a test file. If the test still fails, \
                     this usually turns a regression into a green build.",
                    change.prompt_id.clone(),
                    Some(change.path.clone()),
                ));
            }

            let added: Vec<String> = hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .filter(|line| line.tag == DiffTag::Add && is_skip(&line.text))
                .map(|line| line.text.clone())
                .collect();
            if !added.is_empty() {
                found.push(incident(
                    step,
                    event.t_ms,
                    IncidentKind::TestWeakened,
                    Severity::Critical,
                    "Test skipped",
                    compose_evidence(&added, 240),
                    "A test was skipped or marked todo. Skipped tests report success while checking \
                     nothing, so the suite no longer describes what the code does.",
                    change.prompt_id.clone(),
                    Some(change.path.clone()),
                ));
            }
        }

        // A suite that suddenly reports fewer tests than before is the quietest
        // form of weakening: nothing in the diff looks suspicious.
        //
        // Totals are compared per command, because two different commands run two
        // different suites. A filtered `cargo test --lib adds` after a full run is
        // not a shrinking suite, and neither is jest's four tests followed by
        // node's three; comparing across them reports the shuffle as a critical
        // incident, which is how a detector loses its reader.
        let mut totals: BTreeMap<&str, u32> = BTreeMap::new();
        for event in input.events.iter() {
            let EventKind::TestRun(run) = &event.kind else {
                continue;
            };
            let cmd = run.cmd.trim();
            if let Some(total) = totals.get(cmd).copied() {
                if run.total < total {
                    let delta = total - run.total;
                    found.push(incident(
                        event.seq,
                        event.t_ms,
                        IncidentKind::TestWeakened,
                        Severity::Critical,
                        format!("Test count dropped by {delta}"),
                        format!("{total} → {} tests · {}", run.total, run.cmd.trim()),
                        "The suite shrank between two runs without the total being explained. \
                         Deleted or skipped tests stop guarding the code they covered.",
                        run.prompt_id.clone(),
                        None,
                    ));
                }
            }
            totals.insert(cmd, run.total);
        }

        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Event;

    #[test]
    fn assertions_are_recognised_in_several_ecosystems() {
        assert!(is_assertion("  expect(add(1, 2)).toBe(3);"));
        assert!(is_assertion("    assert_eq!(value, 3);"));
        assert!(is_assertion("self.assertEqual(a, b)"));
        assert!(is_assertion("  REQUIRE(x == 1);"));
    }

    #[test]
    fn ordinary_test_lines_are_not_assertions() {
        assert!(!is_assertion("test('adds numbers', () => {"));
        assert!(!is_assertion("import { describe, it } from 'vitest';"));
    }

    fn run(seq: u64, cmd: &str, total: u32) -> Event {
        Event::new(
            seq,
            seq * 1000,
            EventKind::TestRun(crate::schema::TestRun {
                cmd: cmd.to_string(),
                passed: total,
                failed: 0,
                skipped: 0,
                total,
                ok: true,
                prompt_id: None,
            }),
        )
    }

    fn count_drops(events: &[Event]) -> Vec<String> {
        let input = DetectInput::new(events, &super::super::NoDiffs);
        TestWeakened
            .detect(&input)
            .into_iter()
            .map(|incident| incident.title)
            .collect()
    }

    #[test]
    fn a_smaller_suite_under_the_same_command_is_reported() {
        let events = vec![run(0, "npm test", 42), run(1, "npm test", 41)];
        assert_eq!(count_drops(&events), vec!["Test count dropped by 1"]);
    }

    #[test]
    fn different_commands_are_different_suites() {
        // A filtered run and a run under another test runner both report fewer
        // tests without anything being weakened.
        let events = vec![
            run(0, "npx jest", 4),
            run(1, "node --test js/nodecheck.js", 3),
            run(2, "cargo test", 3),
            run(3, "cargo test --lib adds", 2),
        ];
        assert!(count_drops(&events).is_empty());
    }

    #[test]
    fn a_rerun_of_the_same_command_is_compared_with_its_own_previous_total() {
        let events = vec![
            run(0, "cargo test", 30),
            run(1, "cargo test --lib adds", 2),
            run(2, "cargo test", 30),
            run(3, "cargo test", 29),
        ];
        assert_eq!(count_drops(&events), vec!["Test count dropped by 1"]);
    }

    #[test]
    fn skips_are_recognised() {
        assert!(is_skip("it.skip('later', () => {})"));
        assert!(is_skip("#[ignore]"));
        assert!(is_skip("@pytest.mark.skip(reason=\"flaky\")"));
        assert!(!is_skip("it('runs', () => {})"));
    }
}
