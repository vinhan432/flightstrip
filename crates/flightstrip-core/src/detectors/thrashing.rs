//! The same file rewritten over and over inside one prompt window: the agent
//! (or the prompt) does not know what the target state is.

use super::{incident, DetectInput, Detector};
use crate::schema::{EventKind, Incident, IncidentKind, Severity};
use std::collections::HashMap;

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

        let mut buckets: HashMap<(String, Option<String>), Bucket> = HashMap::new();
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
            if bucket.count < THRESHOLD {
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
