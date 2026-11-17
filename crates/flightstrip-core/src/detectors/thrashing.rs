//! The same file rewritten over and over inside one prompt window: the agent
//! (or the prompt) does not know what the target state is.

use super::{incident, DetectInput, Detector};
use crate::schema::{EventKind, Incident, IncidentKind, Severity};
use std::collections::BTreeMap;

/// Changes to one path within one prompt before it counts as thrashing.
pub const THRESHOLD: u32 = 5;

pub struct Thrashing;

impl Detector for Thrashing {
    fn name(&self) -> &'static str {
        "thrashing"
    }

    fn detect(&self, input: &DetectInput) -> Vec<Incident> {
        struct Bucket {
            count: u32,
            first: u64,
            last_event: u64,
        }

        // BTreeMap, not HashMap: detector output has to be deterministic even
        // before `run_all` sorts it, or a test that compares titles flips.
        let mut buckets: BTreeMap<(String, Option<String>), Bucket> = BTreeMap::new();
        for (step, event) in input.events.iter().enumerate() {
            let EventKind::FileChange(change) = &event.kind else {
                continue;
            };
            let key = (change.path.clone(), change.prompt_id.clone());
            let bucket = buckets.entry(key).or_insert(Bucket {
                count: 0,
                first: step as u64,
                last_event: step as u64,
            });
            bucket.count += 1;
            bucket.last_event = step as u64;
        }

        let mut found = Vec::new();
        for ((path, prompt_id), bucket) in buckets {
            if bucket.count < input.thrash_threshold {
                continue;
            }
            let Some(event) = input.events.get(bucket.last_event as usize) else {
                continue;
            };
            found.push(incident(
                bucket.last_event,
                event.t_ms,
                IncidentKind::Thrashing,
                Severity::Warn,
                format!("{path} rewritten {} times", bucket.count),
                format!(
                    "{} changes to one file inside a single prompt, starting at step {}",
                    bucket.count, bucket.first
                ),
                "Repeated rewrites of the same file usually mean the requirement was ambiguous: \
                 the agent tried several shapes because nothing said which one was right.",
                prompt_id,
                Some(path),
            ));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detectors::test_util::file_event;
    use crate::detectors::NoDiffs;
    use crate::schema::{EventKind, FileOp, PromptSource};

    fn with_prompt(events: &mut Vec<crate::schema::Event>, id: &str) {
        events.push(crate::schema::Event::new(
            events.len() as u64,
            0,
            EventKind::Prompt(crate::schema::Prompt {
                id: id.to_string(),
                text: "go".into(),
                source: PromptSource::Mark,
                agent: None,
            }),
        ));
    }

    fn run(events: &[crate::schema::Event]) -> Vec<Incident> {
        Thrashing.detect(&DetectInput::new(events, &NoDiffs))
    }

    #[test]
    fn five_rewrites_of_one_file_inside_one_prompt_is_warned() {
        let events: Vec<_> = (0..THRESHOLD)
            .map(|index| file_event(index as u64, index as u64 * 10, "src/a.rs", FileOp::Modify))
            .collect();
        let found = run(&events);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].severity, Severity::Warn);
        assert!(found[0].evidence.contains("5 changes"));
    }

    #[test]
    fn four_rewrites_are_still_normal_work() {
        let events: Vec<_> = (0..THRESHOLD - 1)
            .map(|index| file_event(index as u64, index as u64 * 10, "src/a.rs", FileOp::Modify))
            .collect();
        assert!(run(&events).is_empty());
    }

    #[test]
    fn rewrites_split_across_prompts_are_not_thrashing() {
        let mut events = Vec::new();
        with_prompt(&mut events, "p-1");
        for index in 0..3 {
            events.push(file_event(
                events.len() as u64,
                index,
                "src/a.rs",
                FileOp::Modify,
            ));
        }
        with_prompt(&mut events, "p-2");
        for index in 0..3 {
            events.push(file_event(
                events.len() as u64,
                index,
                "src/a.rs",
                FileOp::Modify,
            ));
        }
        let mut attributed = events;
        crate::attributor::attribute(&mut attributed, crate::attributor::DEFAULT_GAP_MS);
        assert!(run(&attributed).is_empty());
    }
}
