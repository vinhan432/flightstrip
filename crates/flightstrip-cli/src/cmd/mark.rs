//! `flightstrip mark`: attach a prompt to the timeline.

use crate::cmd;
use crate::error::{Context, Result};
use flightstrip_core::schema::{EventKind, PromptSource};
use flightstrip_core::session::EventLog;
use std::path::Path;

pub fn run(root: &Path, text: &str) -> Result<()> {
    let id = cmd::active_session(root)?;
    let mut log = EventLog::open(root, &id).context("while opening the session")?;
    let event = log
        .mark_prompt(text, PromptSource::Mark)
        .context("while writing the prompt")?;

    let prompt_id = match &event.kind {
        EventKind::Prompt(prompt) => prompt.id.clone(),
        _ => String::from("?"),
    };
    let first_line: String = text.lines().next().unwrap_or(text).trim().to_string();
    let short = if first_line.chars().count() > 60 {
        format!("{}…", first_line.chars().take(59).collect::<String>())
    } else {
        first_line
    };
    println!("{prompt_id} marked at {}ms: {short}", event.t_ms);
    Ok(())
}
