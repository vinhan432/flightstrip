//! `flightstrip verify`: check a `session.json` against the export contract
//! before it is attached to a review or handed to another tool.

use crate::error::{Context, Result};
use crate::style::Style;
use flightstrip_core::schema::{SessionExport, SCHEMA_VERSION};
use flightstrip_core::time;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Error,
    Warn,
}

struct Issue {
    level: Level,
    message: String,
}

#[derive(Default)]
struct Issues {
    items: Vec<Issue>,
}

impl Issues {
    fn error(&mut self, message: impl Into<String>) {
        self.items.push(Issue {
            level: Level::Error,
            message: message.into(),
        });
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.items.push(Issue {
            level: Level::Warn,
            message: message.into(),
        });
    }
}

pub fn run(path: &Path, style: Style) -> Result<bool> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("while reading {}", path.display()))?;
    let export = match SessionExport::parse(&text) {
        Ok(export) => export,
        Err(error) => {
            println!("{} {}", style.fail("invalid"), path.display());
            println!("  {error}");
            return Ok(false);
        }
    };

    let issues = validate(&export);
    let errors = issues
        .items
        .iter()
        .filter(|i| i.level == Level::Error)
        .count();
    let warnings = issues
        .items
        .iter()
        .filter(|i| i.level == Level::Warn)
        .count();

    println!(
        "{} {} ({} steps, {} prompts, {} incidents)",
        if errors == 0 {
            style.pass("Valid")
        } else {
            style.fail("Invalid")
        },
        path.display(),
        export.steps.len(),
        export.prompts.len(),
        export.incidents.len()
    );
    for issue in &issues.items {
        let label = match issue.level {
            Level::Error => style.fail("error"),
            Level::Warn => style.warn("warn"),
        };
        println!("  {label}  {}", issue.message);
    }
    if issues.items.is_empty() {
        println!("  contract checks passed");
    } else {
        println!("  {errors} error(s), {warnings} warning(s)");
    }
    Ok(errors == 0)
}

fn validate(export: &SessionExport) -> Issues {
    let mut issues = Issues::default();

    if export.version == 0 {
        issues.error("version is 0; the first published schema version is 1");
    } else if export.version != SCHEMA_VERSION {
        issues.warn(format!(
            "version {} is newer than this build ({}); some fields may be unknown",
            export.version, SCHEMA_VERSION
        ));
    }

    if export.meta.id.trim().is_empty() {
        issues.error("meta.id is empty");
    }
    if export.meta.title.trim().is_empty() {
        issues.warn("meta.title is empty");
    }
    if time::parse_rfc3339(&export.meta.started_at).is_none() {
        issues.error(format!(
            "meta.started_at `{}` is not RFC 3339 UTC",
            export.meta.started_at
        ));
    }

    let mut prompt_ids: BTreeSet<&str> = BTreeSet::new();
    for (index, prompt) in export.prompts.iter().enumerate() {
        if prompt.id.trim().is_empty() {
            issues.error(format!("prompts[{index}].id is empty"));
        } else if !prompt_ids.insert(&prompt.id) {
            issues.error(format!("prompts[{index}].id `{}` is duplicated", prompt.id));
        }
        if prompt.step as usize >= export.steps.len() {
            issues.error(format!(
                "prompts[{index}].step {} is outside steps[0..{}]",
                prompt.step,
                export.steps.len()
            ));
        } else if export.steps[prompt.step as usize].kind_name() != "prompt" {
            issues.warn(format!(
                "prompts[{index}].step {} points at a {} event, not the prompt",
                prompt.step,
                export.steps[prompt.step as usize].kind_name()
            ));
        }
    }

    let mut incident_ids: BTreeSet<&str> = BTreeSet::new();
    let mut by_kind: BTreeMap<String, u32> = BTreeMap::new();
    for (index, incident) in export.incidents.iter().enumerate() {
        if incident.id.trim().is_empty() {
            issues.error(format!("incidents[{index}].id is empty"));
        } else if !incident_ids.insert(&incident.id) {
            issues.error(format!(
                "incidents[{index}].id `{}` is duplicated",
                incident.id
            ));
        }
        if incident.step as usize >= export.steps.len() {
            issues.error(format!(
                "incidents[{index}].step {} is outside steps[0..{}]",
                incident.step,
                export.steps.len()
            ));
        }
        if let Some(prompt_id) = &incident.prompt_id {
            if !prompt_ids.contains(prompt_id.as_str()) {
                issues.error(format!(
                    "incidents[{index}].prompt_id `{prompt_id}` does not name a prompt"
                ));
            }
        }
        if incident.evidence.trim().is_empty() {
            issues.warn(format!("incidents[{index}].evidence is empty"));
        }
        if incident.why_it_matters.trim().is_empty() {
            issues.warn(format!("incidents[{index}].why_it_matters is empty"));
        }
        if let Some(path) = &incident.path {
            check_path(path, &mut issues);
        }
        *by_kind
            .entry(incident.kind.as_str().to_string())
            .or_default() += 1;
    }

    for (path, summary) in &export.files {
        check_path(path, &mut issues);
        if summary.churn == 0 {
            issues.warn(format!("files[\"{path}\"].churn is 0"));
        }
        for prompt_id in &summary.prompts {
            if !prompt_ids.contains(prompt_id.as_str()) {
                issues.error(format!(
                    "files[\"{path}\"] names prompt `{prompt_id}`, which is not in prompts"
                ));
            }
        }
    }

    if export.stats.steps != export.steps.len() as u64 {
        issues.warn(format!(
            "stats.steps is {} but steps has {} entries",
            export.stats.steps,
            export.steps.len()
        ));
    }
    if export.stats.incidents_by_kind != by_kind {
        issues.warn(format!(
            "stats.incidents_by_kind does not match the incidents ({} vs {})",
            export.stats.incidents_by_kind.len(),
            by_kind.len()
        ));
    }
    if export.stats.files_touched as usize != export.files.len() {
        issues.warn(format!(
            "stats.files_touched is {} but files has {} entries",
            export.stats.files_touched,
            export.files.len()
        ));
    }

    issues
}

fn check_path(path: &str, issues: &mut Issues) {
    if path.is_empty() {
        issues.error("a file path is empty");
        return;
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        issues.error(format!(
            "path `{path}` is not repo-relative with forward slashes"
        ));
    }
    if path.split('/').any(|part| part == "..") {
        issues.error(format!("path `{path}` escapes the repository"));
    }
}
