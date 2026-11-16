//! New third-party code entering the project. One incident per manifest, listing
//! what was added.
//!
//! Package names are read out of added manifest lines with a parser per
//! ecosystem rather than a regex: each format puts the name somewhere different,
//! and a small function is clearer about where.

use super::{compose_evidence, incident, DetectInput, Detector};
use crate::paths::dependency_file;
use crate::schema::{DiffTag, EventKind, Incident, IncidentKind, Severity};
use std::collections::BTreeSet;

/// Above this many packages in one change, the review burden is worth a warning.
pub const WARN_THRESHOLD: usize = 2;

fn strip_name(text: &str) -> Option<String> {
    let name = text.trim().trim_matches('"').trim().to_string();
    if name.is_empty() || name == "name" || name == "version" {
        return None;
    }
    Some(name)
}

/// `"lodash": "^4.17.21"`
fn npm(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix('"')?;
    let (name, tail) = rest.split_once('"')?;
    // Only `"pkg": "version"` is a dependency; `"scripts": {` is a section.
    let value = tail.trim_start().strip_prefix(':')?.trim_start();
    if !value.starts_with('"') {
        return None;
    }
    strip_name(name)
}

/// `serde = { version = "1" }` / `requests = "^2"`
fn key_value(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('[') {
        return None;
    }
    let (key, tail) = trimmed.split_once('=')?;
    if !tail.trim_start().starts_with('"') && !tail.trim_start().starts_with('{') {
        return None;
    }
    let key = key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return None;
    }
    strip_name(key)
}

/// `requests==2.31.0`, `flask>=2`, and bare `numpy`.
fn pip(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
        return None;
    }
    let end = trimmed
        .find(|c: char| {
            matches!(
                c,
                '=' | '<' | '>' | '~' | '!' | ' ' | '\t' | '[' | ';' | ','
            )
        })
        .unwrap_or(trimmed.len());
    strip_name(&trimmed[..end])
}

/// `github.com/stretchr/testify v1.9.0`
fn go(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let mut parts = trimmed.split_whitespace();
    let name = parts.next()?;
    let version = parts.next()?;
    if !name.contains('.') || !name.contains('/') || !version.starts_with('v') {
        return None;
    }
    strip_name(name)
}

/// `gem "rails", "~> 7.0"`
fn bundler(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix("gem ")?;
    let rest = rest.trim_start().strip_prefix(['"', '\''])?;
    let end = rest.find(['"', '\''])?;
    strip_name(&rest[..end])
}

/// `<artifactId>junit</artifactId>`
fn maven(line: &str) -> Option<String> {
    let start = line.find("<artifactId>")? + "<artifactId>".len();
    let end = line[start..].find("</artifactId>")? + start;
    strip_name(&line[start..end])
}

fn extract(ecosystem: &str, line: &str) -> Option<String> {
    match ecosystem {
        "npm" => npm(line),
        "cargo" | "python" => key_value(line),
        "pip" => pip(line),
        "go" => go(line),
        "bundler" => bundler(line),
        "maven" => maven(line),
        _ => None,
    }
}

/// Package names added by a change, excluding names that also disappeared from
/// the same manifest. A version bump looks like `- "pkg": "1.0"` plus
/// `+ "pkg": "1.1"`; that is an update, not a new dependency.
fn added_packages(hunks: &[crate::schema::DiffHunk], ecosystem: &str) -> BTreeSet<String> {
    let mut removed: BTreeSet<String> = BTreeSet::new();
    let mut added: BTreeSet<String> = BTreeSet::new();
    for line in hunks.iter().flat_map(|h| h.lines.iter()) {
        match line.tag {
            DiffTag::Add => {
                if let Some(name) = extract(ecosystem, &line.text) {
                    added.insert(name);
                }
            }
            DiffTag::Remove => {
                if let Some(name) = extract(ecosystem, &line.text) {
                    removed.insert(name);
                }
            }
            DiffTag::Context => {}
        }
    }
    added.retain(|name| !removed.contains(name));
    added
}

pub struct DependencyAdded;

impl Detector for DependencyAdded {
    fn name(&self) -> &'static str {
        "dependency_added"
    }

    fn detect(&self, input: &DetectInput) -> Vec<Incident> {
        let mut found = Vec::new();

        for (step, event) in input.events.iter().enumerate() {
            let EventKind::FileChange(change) = &event.kind else {
                continue;
            };
            let Some(ecosystem) = dependency_file(&change.path) else {
                continue;
            };
            let hunks = input.diffs.hunks_for(event.seq);
            if hunks.is_empty() {
                continue;
            }

            let packages = added_packages(hunks, ecosystem);
            if packages.is_empty() {
                continue;
            }

            let list: Vec<String> = packages.into_iter().collect();
            let severity = if list.len() > WARN_THRESHOLD {
                Severity::Warn
            } else {
                Severity::Info
            };
            found.push(incident(
                step as u64,
                event.t_ms,
                IncidentKind::DependencyAdded,
                severity,
                format!(
                    "{} package{} added",
                    list.len(),
                    if list.len() == 1 { "" } else { "s" }
                ),
                compose_evidence(&[list.join(", ")], 240),
                "Every dependency is code that runs with your privileges and needs its own \
                 security and maintenance review. Reviewers tend to skip manifest diffs.",
                change.prompt_id.clone(),
                Some(change.path.clone()),
            ));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_names_come_from_quoted_keys() {
        assert_eq!(
            npm("  \"left-pad\": \"^1.3.0\",").as_deref(),
            Some("left-pad")
        );
        assert_eq!(npm("  \"name\": \"app\","), None);
        assert_eq!(npm("  \"version\": \"1.0.0\","), None);
        assert_eq!(npm("  \"scripts\": {"), None);
    }

    #[test]
    fn cargo_and_python_tables() {
        assert_eq!(
            key_value("serde = { version = \"1\" }").as_deref(),
            Some("serde")
        );
        assert_eq!(
            key_value("requests = \"^2.31\"").as_deref(),
            Some("requests")
        );
        assert_eq!(key_value("[dependencies]"), None);
        // The manifest's own metadata is not a dependency.
        assert_eq!(key_value("name = \"flightstrip\""), None);
    }

    #[test]
    fn pip_pins_and_bare_names() {
        assert_eq!(pip("requests==2.31.0").as_deref(), Some("requests"));
        assert_eq!(pip("flask>=2").as_deref(), Some("flask"));
        assert_eq!(pip("numpy").as_deref(), Some("numpy"));
        assert_eq!(pip("-r other.txt"), None);
        assert_eq!(pip("# comment"), None);
    }

    #[test]
    fn go_and_bundler_and_maven() {
        assert_eq!(
            go("github.com/stretchr/testify v1.9.0").as_deref(),
            Some("github.com/stretchr/testify")
        );
        assert_eq!(go("module example.com/x"), None);
        assert_eq!(
            bundler("gem \"rails\", \"~> 7.0\"").as_deref(),
            Some("rails")
        );
        assert_eq!(
            maven("    <artifactId>junit</artifactId>").as_deref(),
            Some("junit")
        );
    }

    #[test]
    fn ordinary_code_lines_are_not_packages() {
        assert_eq!(key_value("let x = 1;"), None);
        assert_eq!(npm("console.log(\"hi\");"), None);
    }

    fn detected(path: &str, hunks: &[(DiffTag, &str)]) -> Vec<Incident> {
        use super::super::test_util::{file_event, StubDiffs};
        let events = [file_event(0, 10, path, crate::schema::FileOp::Modify)];
        let diffs = StubDiffs::new().with(0, hunks);
        let input = DetectInput::new(&events, &diffs);
        DependencyAdded.detect(&input)
    }

    #[test]
    fn a_version_bump_is_not_a_new_dependency() {
        let found = detected(
            "package.json",
            &[
                (DiffTag::Remove, "    \"left-pad\": \"^1.2.0\","),
                (DiffTag::Add, "    \"left-pad\": \"^1.3.0\","),
            ],
        );
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn a_genuinely_new_package_is_reported() {
        let found = detected(
            "package.json",
            &[
                (DiffTag::Context, "  \"dependencies\": {"),
                (DiffTag::Add, "    \"left-pad\": \"^1.3.0\","),
            ],
        );
        assert_eq!(found.len(), 1);
        assert!(found[0].evidence.contains("left-pad"));
    }
}
