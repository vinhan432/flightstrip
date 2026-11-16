//! Secret redaction for `flightstrip export --redact`.
//!
//! Redaction is applied to prompts, commands, notes, evidence and diff lines —
//! never to file *names*, because a reviewer needs to know that `.env` was
//! touched to understand the incident.
//!
//! Redaction is the one place where a regex crate would have been genuinely
//! convenient, but three hand-written passes are not much code and keep the
//! binary small: known token prefixes, `key = value` pairs, and high-entropy
//! runs. Each pass is deliberately conservative — a false negative shows a
//! secret, so patterns match on shape rather than on a specific provider.

use crate::paths::is_secret_path;
use crate::schema::{DiffTag, EventKind, SessionExport};
use std::collections::HashMap;

/// Prefixes that are a credential on their own, with no other context needed.
const TOKEN_PREFIXES: &[&str] = &[
    "sk-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "github_pat_",
    "akia",
    "aiza",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxr-",
    "xoxs-",
];

/// Words that, followed by `:` or `=`, introduce a secret value.
const SECRET_KEYS: &[&str] = &[
    "password", "passwd", "secret", "api_key", "api-key", "apikey", "token", "auth", "bearer",
];

fn is_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'_' | b'-')
}

fn word_boundary_before(bytes: &[u8], index: usize) -> bool {
    index == 0 || !(bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'_')
}

/// Replace `sk-…`, `ghp_…` and friends, plus JWTs.
fn redact_tokens(input: &str) -> String {
    let bytes = input.as_bytes();
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut index = 0usize;

    while index < bytes.len() {
        // A JWT is three base64url segments separated by dots.
        if lower[index..].starts_with("eyj") && word_boundary_before(bytes, index) {
            let mut end = index;
            while end < bytes.len() && (is_token_char(bytes[end]) || bytes[end] == b'.') {
                end += 1;
            }
            if end - index >= 24 && bytes[index..end].contains(&b'.') {
                out.push_str("[REDACTED:token]");
                index = end;
                continue;
            }
        }

        let mut matched = None;
        if word_boundary_before(bytes, index) {
            for prefix in TOKEN_PREFIXES {
                if lower[index..].starts_with(prefix) {
                    matched = Some(prefix.len());
                    break;
                }
            }
        }
        if let Some(prefix_len) = matched {
            let mut end = index + prefix_len;
            while end < bytes.len() && is_token_char(bytes[end]) {
                end += 1;
            }
            // `sk-` on its own, or a word that merely starts with one of these
            // prefixes, is left alone.
            if end - (index + prefix_len) >= 8 {
                out.push_str("[REDACTED:token]");
                index = end;
                continue;
            }
        }

        let ch_len = utf8_len(bytes[index]);
        out.push_str(&input[index..index + ch_len]);
        index += ch_len;
    }
    out
}

fn utf8_len(byte: u8) -> usize {
    match byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// `api_key = "hunter2hunter2"` keeps the key and loses the value.
fn redact_key_values(input: &str) -> String {
    let bytes = input.as_bytes();
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut index = 0usize;

    'outer: while index < bytes.len() {
        if word_boundary_before(bytes, index) {
            for key in SECRET_KEYS {
                if !lower[index..].starts_with(key) {
                    continue;
                }
                let after_key = index + key.len();
                if after_key < bytes.len()
                    && (bytes[after_key].is_ascii_alphanumeric() || bytes[after_key] == b'_')
                {
                    continue;
                }
                // `key : value`, `key=value`, `key = "value"`.
                let mut cursor = after_key;
                while cursor < bytes.len() && bytes[cursor] == b' ' {
                    cursor += 1;
                }
                if cursor >= bytes.len() || !matches!(bytes[cursor], b':' | b'=') {
                    continue;
                }
                cursor += 1;
                while cursor < bytes.len() && bytes[cursor] == b' ' {
                    cursor += 1;
                }
                let quote = if cursor < bytes.len() && matches!(bytes[cursor], b'"' | b'\'') {
                    cursor += 1;
                    true
                } else {
                    false
                };
                let value_start = cursor;
                while cursor < bytes.len()
                    && !bytes[cursor].is_ascii_whitespace()
                    && bytes[cursor] != b'"'
                    && bytes[cursor] != b'\''
                {
                    cursor += 1;
                }
                if cursor - value_start >= 8 {
                    out.push_str(&input[index..value_start]);
                    out.push_str("[REDACTED]");
                    index = cursor;
                    if quote {
                        // Re-emit the closing quote as part of the ordinary text.
                        out.push_str(&input[cursor..cursor]);
                    }
                    continue 'outer;
                }
            }
        }
        let ch_len = utf8_len(bytes[index]);
        out.push_str(&input[index..index + ch_len]);
        index += ch_len;
    }
    out
}

fn looks_like_hash(token: &str) -> bool {
    token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

fn looks_like_url(token: &str) -> bool {
    token.contains("://") || token.starts_with("www.") || token.contains('@')
}

/// A path is not a secret. Neither is a hash, a URL, or anything with too few
/// distinct characters to carry 4.5 bits/char of real information.
fn high_entropy_candidate(token: &str) -> bool {
    token.chars().count() >= 24
        && !looks_like_hash(token)
        && !looks_like_url(token)
        && !token.contains('/')
        && shannon_entropy(token) > 4.5
}

/// Replace long random-looking runs. This is the pass that catches credentials
/// with no recognisable prefix.
fn redact_high_entropy(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if is_token_char(bytes[index]) {
            let mut end = index;
            while end < bytes.len() && is_token_char(bytes[end]) {
                end += 1;
            }
            let token = &input[index..end];
            if high_entropy_candidate(token) {
                out.push_str("[REDACTED:high-entropy]");
            } else {
                out.push_str(token);
            }
            index = end;
            continue;
        }
        let ch_len = utf8_len(bytes[index]);
        out.push_str(&input[index..index + ch_len]);
        index += ch_len;
    }
    out
}

pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut counts: HashMap<char, usize> = HashMap::new();
    let mut len = 0usize;
    for ch in s.chars() {
        *counts.entry(ch).or_insert(0) += 1;
        len += 1;
    }
    let len = len as f64;
    counts
        .values()
        .map(|&count| {
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

pub fn redact_text(input: &str) -> String {
    let step1 = redact_tokens(input);
    let step2 = redact_key_values(&step1);
    redact_high_entropy(&step2)
}

/// Redact every free-text field that could carry a credential.
pub fn redact_export(export: &mut SessionExport) {
    for prompt in export.prompts.iter_mut() {
        prompt.text = redact_text(&prompt.text);
    }
    for step in export.steps.iter_mut() {
        // A .env file's whole point is values that must never leave the
        // machine. Redacting line by line is not enough: a line like
        // `FOO=bar` has no recognisable secret shape, but it is still the
        // contents of a secret-bearing file. Drop the hunks entirely and keep
        // the path, which is what the reviewer needs to know.
        if let EventKind::FileChange(change) = &step.kind {
            if is_secret_path(&change.path) {
                step.hunks = None;
                continue;
            }
        }
        match &mut step.kind {
            EventKind::Prompt(p) => p.text = redact_text(&p.text),
            EventKind::Command(c) => c.cmd = redact_text(&c.cmd),
            EventKind::TestRun(t) => t.cmd = redact_text(&t.cmd),
            EventKind::Note(n) => n.text = redact_text(&n.text),
            _ => {}
        }
        if let Some(hunks) = step.hunks.as_mut() {
            for hunk in hunks.iter_mut() {
                for line in hunk.lines.iter_mut() {
                    if line.tag != DiffTag::Context || line.text.contains('=') {
                        line.text = redact_text(&line.text);
                    }
                }
            }
        }
    }
    for incident in export.incidents.iter_mut() {
        incident.evidence = redact_text(&incident.evidence);
        incident.title = redact_text(&incident.title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_token_prefixes() {
        let out = redact_text("export GITHUB=ghp_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1234");
        assert!(!out.contains("ghp_aaaa"), "{out}");
        assert!(out.contains("[REDACTED"));
    }

    #[test]
    fn keeps_the_key_and_drops_the_value() {
        let out = redact_text("api_key = \"abcdef1234567890\"");
        assert!(out.contains("api_key"), "{out}");
        assert!(!out.contains("abcdef1234567890"), "{out}");
    }

    #[test]
    fn leaves_ordinary_code_alone() {
        let src = "let path = PathBuf::from(\"src/main.rs\");";
        assert_eq!(redact_text(src), src);
    }

    #[test]
    fn redundant_hex_digests_are_not_redacted() {
        // A content hash in an event is not a secret, and redacting it would
        // make the strip chart unreadable.
        let hash = "a".repeat(32);
        assert_eq!(
            redact_text(&format!("after_hash: {hash}")),
            format!("after_hash: {hash}")
        );
    }

    #[test]
    fn entropy_of_repeated_text_is_low() {
        assert!(shannon_entropy("aaaaaaaaaaaaaaaaaaaaaaaa") < 1.0);
        assert!(shannon_entropy("q7Zx!9pLz#4mWk@2RtYv") > 4.0);
    }

    #[test]
    fn every_required_token_shape_is_redacted() {
        for token in [
            "sk-abcdefghijklmnop1234",
            "ghp_abcdefghijklmnop1234",
            "AKIAIOSFODNN7EXAMPLE",
            "AIzaSyABCDEFGHIJKLMNOPQRSTUV",
            "xoxb-1234-abcdefghijklmnop",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.signature123",
        ] {
            let out = redact_text(&format!("token={token}"));
            assert!(!out.contains(token), "{token} survived: {out}");
            assert!(out.contains("REDACTED"), "{token}: {out}");
        }
    }

    #[test]
    fn dotenv_hunks_are_removed_entirely() {
        use crate::schema::*;
        let mut export = SessionExport {
            version: 1,
            meta: SessionMeta {
                id: "s".into(),
                title: "t".into(),
                agent: None,
                started_at: "2026-01-01T00:00:00.000Z".into(),
                duration_ms: 1,
                os: "windows".into(),
                repo_name: "r".into(),
                label: SessionLabel::Recorded,
            },
            prompts: Vec::new(),
            steps: vec![ExportStep {
                seq: 0,
                t_ms: 1,
                kind: EventKind::FileChange(FileChange {
                    path: ".env".into(),
                    op: FileOp::Modify,
                    before_hash: None,
                    after_hash: None,
                    lines_added: 1,
                    lines_removed: 0,
                    prompt_id: None,
                }),
                hunks: Some(vec![DiffHunk {
                    old_start: 1,
                    new_start: 1,
                    lines: vec![DiffLine {
                        tag: DiffTag::Add,
                        text: "INNOCUOUS=value".into(),
                    }],
                }]),
            }],
            incidents: Vec::new(),
            files: std::collections::BTreeMap::new(),
            stats: SessionStats::default(),
        };
        redact_export(&mut export);
        assert!(export.steps[0].hunks.is_none());
    }
}
