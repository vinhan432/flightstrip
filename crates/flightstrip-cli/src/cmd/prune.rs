//! `flightstrip prune`: reclaim disk without deleting the current recording.

use crate::cmd;
use crate::error::{Context, Result};
use crate::style::Style;
use flightstrip_core::hash::is_hash;
use flightstrip_core::schema::EventKind;
use flightstrip_core::session;
use flightstrip_core::time;
use std::collections::BTreeSet;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

pub fn run(
    root: &Path,
    keep: usize,
    older_than_days: Option<u64>,
    dry_run: bool,
    yes: bool,
    style: Style,
) -> Result<()> {
    let active = session::read_active(root).filter(|a| session::pid_alive(a.pid));
    let active_id = active.as_ref().map(|a| a.id.clone());
    let mut sessions = cmd::all_sessions(root);
    sessions.sort();

    let mut doomed: BTreeSet<PathBuf> = BTreeSet::new();
    if sessions.len() > keep {
        for dir in &sessions[..sessions.len() - keep] {
            doomed.insert(dir.clone());
        }
    }
    if let Some(days) = older_than_days {
        let cutoff = time::now_ms().saturating_sub(days.saturating_mul(86_400_000));
        for dir in &sessions {
            let started = session::read_record(dir)
                .and_then(|record| time::parse_rfc3339(&record.started_at))
                .or_else(|| {
                    dir.file_name()
                        .and_then(|name| session::parse_started_at(&name.to_string_lossy()))
                })
                .unwrap_or(0);
            if started < cutoff {
                doomed.insert(dir.clone());
            }
        }
    }
    if let Some(active_id) = &active_id {
        doomed.retain(|dir| {
            dir.file_name()
                .map(|name| name.to_string_lossy() != active_id.as_str())
                .unwrap_or(true)
        });
    }

    // Objects referenced by a session that stays are not orphans. Compute this
    // after the session choice so deleting a session can orphan its snapshots.
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    for dir in &sessions {
        if doomed.contains(dir) {
            continue;
        }
        for event in session::read_events(dir).unwrap_or_default() {
            if let EventKind::FileChange(change) = event.kind {
                if let Some(hash) = change.before_hash {
                    referenced.insert(hash);
                }
                if let Some(hash) = change.after_hash {
                    referenced.insert(hash);
                }
            }
        }
    }

    let objects_dir = session::state_dir(root).join("objects");
    let mut orphaned: Vec<PathBuf> = Vec::new();
    if objects_dir.is_dir() {
        for shard in std::fs::read_dir(&objects_dir)
            .with_context(|| format!("while reading {}", objects_dir.display()))?
            .flatten()
        {
            if !shard.path().is_dir() {
                continue;
            }
            for object in std::fs::read_dir(shard.path())
                .with_context(|| format!("while reading {}", shard.path().display()))?
                .flatten()
            {
                let path = object.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let hash = name.strip_suffix(".snap").unwrap_or("");
                if hash.is_empty() || !is_hash(hash) {
                    continue;
                }
                if !referenced.contains(hash) {
                    orphaned.push(path);
                }
            }
        }
    }
    orphaned.sort();

    if doomed.is_empty() && orphaned.is_empty() {
        println!("{}", style.pass("Nothing to prune."));
        return Ok(());
    }

    let total_bytes: u64 = doomed.iter().map(|dir| directory_size(dir)).sum::<u64>()
        + orphaned
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len())
            .sum::<u64>();

    println!(
        "{} {} session(s) and {} orphaned object(s), about {}",
        if dry_run { "Would delete" } else { "Delete" },
        doomed.len(),
        orphaned.len(),
        human_bytes(total_bytes)
    );
    for dir in &doomed {
        println!(
            "  session {}",
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        );
    }
    for path in orphaned.iter().take(10) {
        println!(
            "  object  {}",
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        );
    }
    if orphaned.len() > 10 {
        println!("  … and {} more objects", orphaned.len() - 10);
    }
    if dry_run {
        println!("{}", style.info("Dry run: nothing was deleted."));
        return Ok(());
    }
    if !yes {
        confirm()?;
    }

    for dir in &doomed {
        std::fs::remove_dir_all(dir)
            .with_context(|| format!("while deleting {}", dir.display()))?;
    }
    let mut removed_objects = 0usize;
    for path in &orphaned {
        if std::fs::remove_file(path).is_ok() {
            removed_objects += 1;
        }
    }
    println!(
        "{} {} session(s), {} object(s)",
        style.pass("Deleted"),
        doomed.len(),
        removed_objects
    );
    Ok(())
}

fn confirm() -> Result<()> {
    if !std::io::stdin().is_terminal() {
        crate::bail!(
            "refusing to delete without confirmation on non-interactive stdin; \
             pass --yes or use --dry-run"
        );
    }
    print!("Continue? [y/N] ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .context("while reading the confirmation")?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        crate::bail!("aborted; nothing was deleted")
    }
}

fn directory_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
