//! Prompt → change attribution. Deliberately simple, and documented as a
//! heuristic in the README: whatever the agent was told most recently is
//! credited with what happened next, until that prompt goes stale.

use crate::schema::{Event, EventKind};

/// A change that lands more than this long after the last prompt, with no
/// command in between, is assumed to be manual work and loses its attribution.
pub const DEFAULT_GAP_MS: u64 = 5 * 60 * 1000;

pub fn attribute(events: &mut [Event], gap_ms: u64) {
    let mut current: Option<String> = None;
    let mut last_prompt_t: Option<u64> = None;
    let mut commands_since_prompt: u32 = 0;

    for event in events.iter_mut() {
        match &event.kind {
            EventKind::Prompt(p) => {
                current = Some(p.id.clone());
                last_prompt_t = Some(event.t_ms);
                commands_since_prompt = 0;
            }
            EventKind::Command(_) => {
                commands_since_prompt = commands_since_prompt.saturating_add(1)
            }
            _ => {}
        }

        if !event.is_attributable() {
            continue;
        }

        let stale = match last_prompt_t {
            Some(t) => event.t_ms.saturating_sub(t) > gap_ms && commands_since_prompt == 0,
            None => true,
        };
        event.set_prompt_id(if stale { None } else { current.clone() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    fn fc(path: &str) -> EventKind {
        EventKind::FileChange(FileChange {
            path: path.into(),
            op: FileOp::Modify,
            before_hash: None,
            after_hash: None,
            lines_added: 1,
            lines_removed: 0,
            prompt_id: None,
        })
    }

    fn prompt(id: &str, text: &str) -> EventKind {
        EventKind::Prompt(Prompt {
            id: id.into(),
            text: text.into(),
            source: PromptSource::Mark,
            agent: None,
        })
    }

    #[test]
    fn change_after_prompt_is_credited_to_it() {
        let mut events = vec![
            Event::new(0, 0, prompt("p-1", "add retry")),
            Event::new(1, 100, fc("src/a.rs")),
            Event::new(2, 200, fc("src/b.rs")),
        ];
        attribute(&mut events, DEFAULT_GAP_MS);
        assert_eq!(events[1].prompt_id(), Some("p-1"));
        assert_eq!(events[2].prompt_id(), Some("p-1"));
    }

    #[test]
    fn second_prompt_takes_over() {
        let mut events = vec![
            Event::new(0, 0, prompt("p-1", "a")),
            Event::new(1, 10, fc("x.rs")),
            Event::new(2, 20, prompt("p-2", "b")),
            Event::new(3, 30, fc("y.rs")),
        ];
        attribute(&mut events, DEFAULT_GAP_MS);
        assert_eq!(events[1].prompt_id(), Some("p-1"));
        assert_eq!(events[3].prompt_id(), Some("p-2"));
    }

    #[test]
    fn stale_prompt_loses_attribution() {
        let mut events = vec![
            Event::new(0, 0, prompt("p-1", "a")),
            Event::new(1, DEFAULT_GAP_MS + 1, fc("x.rs")),
        ];
        attribute(&mut events, DEFAULT_GAP_MS);
        assert_eq!(events[1].prompt_id(), None);
    }

    #[test]
    fn recent_command_keeps_attribution_alive() {
        let mut events = vec![
            Event::new(0, 0, prompt("p-1", "a")),
            Event::new(
                1,
                DEFAULT_GAP_MS + 1,
                EventKind::Command(Command {
                    cmd: "cargo test".into(),
                    exit_code: Some(0),
                    duration_ms: Some(5),
                    source: CommandSource::Exec,
                    cwd: None,
                    prompt_id: None,
                }),
            ),
            Event::new(2, DEFAULT_GAP_MS + 2, fc("x.rs")),
        ];
        attribute(&mut events, DEFAULT_GAP_MS);
        assert_eq!(events[2].prompt_id(), Some("p-1"));
    }
}
