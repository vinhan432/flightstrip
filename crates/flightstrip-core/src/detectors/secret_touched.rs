//! Credentials that were read, written or printed.
//!
//! Evidence is redacted *before* it is put in the incident, so recording a
//! secret does not become a second way to leak it.

use super::{compose_evidence, incident, DetectInput, Detector};
use crate::paths::is_secret_path;
use crate::redact::{redact_text, shannon_entropy};
use crate::schema::{DiffTag, EventKind, Incident, IncidentKind, Severity};

const SECRET_NAMES: &[&str] = &[
    "api_key",
    "api-key",
    "apikey",
    "secret",
    "password",
    "passwd",
    "token",
    "private_key",
    "private-key",
    "access_key",
    "access-key",
];

/// `api_key = "…"` with a value long enough to be a credential rather than a
/// placeholder. Matched by hand: find the name, step over the separator, then
/// measure the value.
fn looks_like_assignment(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    for name in SECRET_NAMES {
        let mut from = 0usize;
        while let Some(found) = lower[from..].find(name) {
            let start = from + found;
            let after = start + name.len();
            let boundary_before = start == 0
                || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
            if boundary_before {
                let mut cursor = after;
                while cursor < bytes.len() && bytes[cursor] == b' ' {
                    cursor += 1;
                }
                if cursor < bytes.len() && matches!(bytes[cursor], b':' | b'=') {
                    cursor += 1;
                    while cursor < bytes.len() && bytes[cursor] == b' ' {
                        cursor += 1;
                    }
                    if cursor < bytes.len() && matches!(bytes[cursor], b'"' | b'\'') {
                        cursor += 1;
                    }
                    let value_start = cursor;
                    while cursor < bytes.len()
                        && (bytes[cursor].is_ascii_alphanumeric()
                            || matches!(bytes[cursor], b'+' | b'/' | b'=' | b'_' | b'-'))
                    {
                        cursor += 1;
                    }
                    if cursor - value_start >= 16 {
                        return true;
                    }
                }
            }
            from = after;
        }
    }
    false
}

fn looks_like_noise(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("http://")
        || lower.contains("https://")
        || lower.contains("example")
        || lower.contains("placeholder")
        || lower.contains("your_")
        || lower.contains("xxxx")
        || lower.contains("process.env")
        || lower.contains("std::env")
        || lower.contains("env::var")
}

/// A long opaque word inside a changed line. Conservative on purpose: normal
/// code, hashes and paths must not be reported as leaks.
fn suspicious_entropy(line: &str) -> bool {
    if looks_like_noise(line) {
        return false;
    }
    line.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';')
        .any(|token| {
            token.chars().count() >= 24
                && token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+/=_-".contains(&b))
                && !(token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit()))
                && !token.contains('/')
                && shannon_entropy(token) > 4.5
        })
}

pub struct SecretTouched;

impl Detector for SecretTouched {
    fn name(&self) -> &'static str {
        "secret_touched"
    }

    fn detect(&self, input: &DetectInput) -> Vec<Incident> {
        let mut found = Vec::new();

        for (step, event) in input.events.iter().enumerate() {
            let EventKind::FileChange(change) = &event.kind else {
                continue;
            };
            let step = step as u64;

            if is_secret_path(&change.path) {
                found.push(incident(
                    step,
                    event.t_ms,
                    IncidentKind::SecretTouched,
                    Severity::Critical,
                    format!("{} touched", change.path),
                    change.path.clone(),
                    "Secret-bearing files must stay out of the diff. If this one is tracked, the \
                     credential is already public the moment the branch is pushed.",
                    change.prompt_id.clone(),
                    Some(change.path.clone()),
                ));
                continue;
            }

            let hunks = input.diffs.hunks_for(event.seq);
            if hunks.is_empty() {
                continue;
            }

            let matched: Vec<String> = hunks
                .iter()
                .flat_map(|h| h.lines.iter())
                .filter(|line| line.tag == DiffTag::Add)
                .filter(|line| looks_like_assignment(&line.text) || suspicious_entropy(&line.text))
                .map(|line| redact_text(&line.text))
                .collect();

            if !matched.is_empty() {
                found.push(incident(
                    step,
                    event.t_ms,
                    IncidentKind::SecretTouched,
                    Severity::Critical,
                    "Credential-shaped value added",
                    compose_evidence(&matched, 240),
                    "This line looks like a literal credential. If it is real, rotate it: removing \
                     it in a later commit does not remove it from history.",
                    change.prompt_id.clone(),
                    Some(change.path.clone()),
                ));
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_values_under_secret_names_are_found() {
        assert!(looks_like_assignment(
            "const api_key = \"AKIAIOSFODNN7EXAMPLEKEY\";"
        ));
        assert!(looks_like_assignment("password: hunter2hunter2hunter2"));
    }

    #[test]
    fn short_or_indirect_values_are_not() {
        assert!(!looks_like_assignment("let password = \"\";"));
        assert!(!looks_like_assignment("const token = process.env.TOKEN;"));
        assert!(!looks_like_assignment("token: string,"));
    }

    #[test]
    fn urls_and_examples_are_noise() {
        assert!(!suspicious_entropy(
            "// see https://example.com/docs/abcdefghijklmnopqrstuvwxyz"
        ));
        assert!(!suspicious_entropy(
            "let path = \"src/components/Button.tsx\";"
        ));
        assert!(!suspicious_entropy(
            "let hash = \"9f2c1d4a8b7e6f5a4c3b2d1e0f9a8b7c\";"
        ));
    }
}
