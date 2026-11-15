//! `session.json` → the markdown trust summary that gets pasted into a PR.
//!
//! Plain markdown on purpose: it has to survive GitHub's renderer, a terminal,
//! and a code review comment box.

use crate::schema::{EventKind, IncidentKind, SessionExport, Severity};
use crate::time::human_duration;
use std::collections::BTreeSet;
use std::fmt::Write as _;

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn pass_fail(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "pass",
        Some(false) => "**fail**",
        None => "no runs",
    }
}

pub fn render(session: &SessionExport) -> String {
    let mut out = String::new();
    let critical = session
        .incidents
        .iter()
        .filter(|i| i.severity == Severity::Critical)
        .count();
    let warn = session
        .incidents
        .iter()
        .filter(|i| i.severity == Severity::Warn)
        .count();
    let info = session
        .incidents
        .iter()
        .filter(|i| i.severity == Severity::Info)
        .count();

    let agent = session
        .meta
        .agent
        .clone()
        .unwrap_or_else(|| "unknown".into());

    let _ = writeln!(out, "# Flightstrip Report: {}", session.meta.title);
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "**Duration:** {} | **Prompts:** {} | **Agent:** {}",
        human_duration(session.meta.duration_ms),
        session.prompts.len(),
        agent
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "## Verdict");
    let _ = writeln!(
        out,
        "{critical} critical · {warn} warning{} · {info} info",
        if warn == 1 { "" } else { "s" }
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "## Incidents");
    if session.incidents.is_empty() {
        let _ = writeln!(
            out,
            "Nothing detected. That means nothing matched a rule — not that the change is safe."
        );
    } else {
        let _ = writeln!(out, "| Step | Kind | Severity | Evidence |");
        let _ = writeln!(out, "|------|------|----------|----------|");
        for incident in &session.incidents {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                incident.step,
                incident.kind.as_str(),
                incident.severity.as_str(),
                cell(&incident.evidence)
            );
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Tests");
    let weakened = session
        .stats
        .incidents_by_kind
        .get(IncidentKind::TestWeakened.as_str())
        .copied()
        .unwrap_or(0);
    let _ = writeln!(
        out,
        "- First run: {}",
        pass_fail(session.stats.tests_first_ok)
    );
    let _ = writeln!(
        out,
        "- Last run: {}",
        pass_fail(session.stats.tests_last_ok)
    );
    let _ = writeln!(
        out,
        "- Weakened/skipped: {weakened} ({} test runs total)",
        session.stats.test_runs
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "## Commands Run");
    let commands: Vec<String> = session
        .steps
        .iter()
        .filter_map(|step| match &step.kind {
            EventKind::Command(c) => Some(format!(
                "- `{}` → {}",
                c.cmd.trim(),
                c.exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "?".into())
            )),
            _ => None,
        })
        .collect();
    if commands.is_empty() {
        let _ = writeln!(out, "none");
    } else {
        for line in commands {
            let _ = writeln!(out, "{line}");
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Dependencies Added");
    let deps: Vec<&str> = session
        .incidents
        .iter()
        .filter(|i| i.kind == IncidentKind::DependencyAdded)
        .map(|i| i.evidence.as_str())
        .collect();
    if deps.is_empty() {
        let _ = writeln!(out, "none");
    } else {
        for dep in deps {
            let _ = writeln!(out, "- {dep}");
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Files Touched (per prompt)");
    let mut any = false;
    for prompt in &session.prompts {
        let files: BTreeSet<&str> = session
            .files
            .iter()
            .filter(|(_, summary)| summary.prompts.iter().any(|p| p == &prompt.id))
            .map(|(path, _)| path.as_str())
            .collect();
        if files.is_empty() {
            continue;
        }
        any = true;
        let _ = writeln!(
            out,
            "- {}: {}",
            prompt.id,
            files.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    if !any {
        let _ = writeln!(out, "none");
    }
    let unattributed: BTreeSet<&str> = session
        .files
        .iter()
        .filter(|(_, summary)| summary.prompts.is_empty())
        .map(|(path, _)| path.as_str())
        .collect();
    if !unattributed.is_empty() {
        let _ = writeln!(
            out,
            "- (no prompt): {}",
            unattributed.into_iter().collect::<Vec<_>>().join(", ")
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "---");
    let _ = writeln!(
        out,
        "_Attribution is heuristic: a change is credited to the most recent prompt unless it \
arrived more than five minutes later with no command in between. Detectors are deterministic \
rules, not a judgement of correctness; no model was involved. Session `{}`._",
        session.meta.id
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;
    use std::collections::BTreeMap;

    fn sample() -> SessionExport {
        SessionExport {
            version: 1,
            meta: SessionMeta {
                id: "s1".into(),
                title: "Fixture".into(),
                agent: Some("claude".into()),
                started_at: "2026-01-01T00:00:00.000Z".into(),
                duration_ms: 65_000,
                os: "windows".into(),
                repo_name: "repo".into(),
                label: SessionLabel::Fixture,
            },
            prompts: vec![],
            steps: vec![],
            incidents: vec![Incident {
                id: "i-0".into(),
                kind: IncidentKind::TestWeakened,
                severity: Severity::Critical,
                step: 3,
                t_ms: 1000,
                title: "assertion removed".into(),
                evidence: "expect(a).toBe(1)".into(),
                why_it_matters: "why".into(),
                prompt_id: None,
                path: None,
            }],
            files: BTreeMap::new(),
            stats: SessionStats {
                test_runs: 2,
                tests_first_ok: Some(true),
                tests_last_ok: Some(false),
                ..Default::default()
            },
        }
    }

    #[test]
    fn renders_the_essentials() {
        let md = render(&sample());
        assert!(md.starts_with("# Flightstrip Report: Fixture"));
        assert!(md.contains("1 critical · 0 warnings · 0 info"));
        assert!(md.contains("| 3 | test_weakened | critical |"));
        assert!(md.contains("- First run: pass"));
        assert!(md.contains("- Last run: **fail**"));
        assert!(md.contains("1m 05s"));
    }

    #[test]
    fn durations_read_like_a_person_wrote_them() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(9_000), "9s");
        assert_eq!(human_duration(125_000), "2m 05s");
        assert_eq!(human_duration(3_780_000), "1h 03m");
    }
}
