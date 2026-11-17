//! `flightstrip status`: what is recording right now.

use crate::error::{Context, Result};
use crate::style::{self, Style};
use flightstrip_core::json;
use flightstrip_core::schema::{Event, EventKind};
use flightstrip_core::session::{self, ActiveRef};
use flightstrip_core::time;
use std::path::Path;

pub fn run(root: &Path, as_json: bool, quiet: bool, style: Style) -> Result<()> {
    let mut stale: Option<ActiveRef> = None;
    let active = match session::read_active(root) {
        Some(active) if session::pid_alive(active.pid) => Some(active),
        Some(active) => {
            session::clear_active(root);
            stale = Some(active);
            None
        }
        None => None,
    };

    let Some(active) = active else {
        if as_json {
            let stale_json = match &stale {
                Some(stale) => json::obj(vec![
                    ("id", json::Json::from(&stale.id)),
                    ("pid", json::Json::from(stale.pid)),
                ]),
                None => json::Json::Null,
            };
            style::emit(&json::to_pretty(&json::obj(vec![
                ("active", json::Json::Null),
                ("stale", stale_json),
            ])));
        } else if quiet {
            println!("idle");
        } else {
            style::emit(&style.warn("No session is recording."));
            if let Some(stale) = &stale {
                println!(
                    "  cleared a stale pointer for {} (pid {} is gone)",
                    stale.id, stale.pid
                );
            }
            println!("  start one with: flightstrip watch");
        }
        return Ok(());
    };

    let dir = session::session_dir(root, &active.id);
    let record = session::read_record(&dir);
    let events = session::read_events(&dir).context("while reading the active session")?;
    let started_ms = record
        .as_ref()
        .and_then(|r| time::parse_rfc3339(&r.started_at))
        .or_else(|| session::parse_started_at(&active.id))
        .unwrap_or_else(time::now_ms);
    let uptime = time::now_ms().saturating_sub(started_ms);

    let files = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::FileChange(_)))
        .count();
    let commands = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::Command(_)))
        .count();
    let tests = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::TestRun(_)))
        .count();
    let prompts = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::Prompt(_)))
        .count();
    let last_prompt = events.iter().rev().find_map(|event| match &event.kind {
        EventKind::Prompt(prompt) => Some((prompt.id.clone(), prompt.text.clone())),
        _ => None,
    });
    let recent: Vec<&Event> = events
        .iter()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    if as_json {
        let prompt_json = match &last_prompt {
            Some((id, text)) => json::obj(vec![
                ("id", json::Json::from(id)),
                ("text", json::Json::from(text)),
            ]),
            None => json::Json::Null,
        };
        let recent_json = json::Json::Arr(
            recent
                .iter()
                .map(|event| {
                    json::obj(vec![
                        ("seq", json::Json::from(event.seq)),
                        ("t_ms", json::Json::from(event.t_ms)),
                        ("kind", json::Json::from(event.kind_name())),
                        ("summary", json::Json::from(event_summary(event))),
                    ])
                })
                .collect(),
        );
        style::emit(&json::to_pretty(&json::obj(vec![
            ("active", json::Json::Bool(true)),
            ("stale", json::Json::Null),
            (
                "session",
                json::obj(vec![
                    ("id", json::Json::from(&active.id)),
                    ("pid", json::Json::from(active.pid)),
                    (
                        "title",
                        match &record {
                            Some(record) => json::Json::from(&record.title),
                            None => json::Json::Null,
                        },
                    ),
                    (
                        "agent",
                        match record.as_ref().and_then(|r| r.agent.clone()) {
                            Some(agent) => json::Json::from(agent),
                            None => json::Json::Null,
                        },
                    ),
                    (
                        "started_at",
                        json::Json::from(time::format_rfc3339(started_ms)),
                    ),
                    ("uptime_ms", json::Json::from(uptime)),
                ]),
            ),
            (
                "counts",
                json::obj(vec![
                    ("events", json::Json::from(events.len())),
                    ("files", json::Json::from(files)),
                    ("commands", json::Json::from(commands)),
                    ("tests", json::Json::from(tests)),
                    ("prompts", json::Json::from(prompts)),
                ]),
            ),
            ("last_prompt", prompt_json),
            ("recent", recent_json),
        ])));
        return Ok(());
    }

    if quiet {
        println!(
            "{} {} {} events",
            active.id,
            time::human_duration(uptime),
            events.len()
        );
        return Ok(());
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{} {} (pid {}, up {})\n",
        style.green("REC"),
        style.bold(&active.id),
        active.pid,
        time::human_duration(uptime)
    ));
    if let Some(record) = &record {
        out.push_str(&format!("  title   {}\n", record.title));
        out.push_str(&format!(
            "  agent   {}\n",
            record.agent.as_deref().unwrap_or("unspecified")
        ));
    }
    out.push_str(&format!(
        "  events  {} ({} file changes, {} commands, {} test runs, {} prompts)\n",
        events.len(),
        files,
        commands,
        tests,
        prompts
    ));
    match &last_prompt {
        Some((id, text)) => out.push_str(&format!(
            "  prompt  {} {}\n",
            style.cyan(id),
            truncate(text, 80)
        )),
        None => out.push_str("  prompt  none marked yet\n"),
    }
    out.push_str("  recent\n");
    if recent.is_empty() {
        out.push_str("    (no events yet)\n");
    } else {
        for event in recent {
            out.push_str(&format!(
                "    {:>7}  {:<12} {}\n",
                format!("{}ms", event.t_ms),
                event.kind_name(),
                event_summary(event)
            ));
        }
    }
    style::emit(&out);
    Ok(())
}

fn event_summary(event: &Event) -> String {
    match &event.kind {
        EventKind::FileChange(change) => format!(
            "{} {} (+{} -{})",
            change.op.as_str(),
            change.path,
            change.lines_added,
            change.lines_removed
        ),
        EventKind::Command(command) => format!(
            "{} → {}",
            truncate(&command.cmd, 60),
            command
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "?".into())
        ),
        EventKind::TestRun(run) => format!(
            "{} · {} passed / {} failed",
            truncate(&run.cmd, 50),
            run.passed,
            run.failed
        ),
        EventKind::Prompt(prompt) => truncate(&prompt.text, 70),
        EventKind::Note(note) => truncate(&note.text, 70),
        EventKind::SessionStart(start) => start.title.clone(),
        EventKind::SessionEnd => "session ended".into(),
    }
}

fn truncate(text: &str, max: usize) -> String {
    let flat = text.replace(['\r', '\n'], " ");
    if flat.chars().count() <= max {
        flat
    } else {
        format!("{}…", flat.chars().take(max - 1).collect::<String>())
    }
}
