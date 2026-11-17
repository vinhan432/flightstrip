//! `flightstrip mark`: attach a prompt to the timeline.

use crate::cmd;
use crate::error::{Context, Result};
use crate::style::Style;
use flightstrip_core::schema::{EventKind, PromptSource};
use flightstrip_core::session::EventLog;
use std::path::Path;

pub fn run(root: &Path, text: &str, agent: Option<String>, style: Style) -> Result<()> {
    let id = cmd::active_session(root)?;
    let mut log = EventLog::open(root, &id).context("while opening the session")?;
    let event = log
        .mark_prompt(text, PromptSource::Mark, agent.clone())
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
    let tag = agent
        .as_deref()
        .map(|name| format!(" · agent {}", style.cyan(name)))
        .unwrap_or_default();
    println!(
        "{} {} marked at {}ms: {}{}",
        style.pass(&prompt_id),
        style.dim("prompt"),
        event.t_ms,
        short,
        tag
    );
    Ok(())
}
