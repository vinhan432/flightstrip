//! Path hygiene. Every path that reaches an event is repo-relative and uses
//! forward slashes, because the same `session.json` is read on macOS and Linux
//! after being recorded on Windows.
//!
//! The `.gitignore` matcher is written here instead of using the `ignore` crate.
//! It covers the syntax that actually appears in repositories — comments, `!`
//! negation, trailing `/` for directories, leading `/` to anchor, `*`, `**`, `?`
//! — and ignores the parts that do not (nested `.gitignore` files, `[a-z]`
//! character classes). The root `.gitignore` is the one that excludes build
//! output, which is all the recorder needs.

use std::path::{Path, PathBuf};

/// Directories and files that never belong in a recording.
const IGNORED_SEGMENTS: &[&str] = &[
    ".flightstrip",
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "out",
    "__pycache__",
    ".venv",
    "venv",
    ".mypy_cache",
    ".pytest_cache",
    ".turbo",
    ".svelte-kit",
    "coverage",
];

/// Content-free history files that would otherwise fill the tape.
const IGNORED_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];

/// Editor scratch files. These are noise from atomic saves, not work.
const IGNORED_SUFFIXES: &[&str] = &[
    ".tmp",
    ".temp",
    ".swp",
    ".swx",
    ".swo",
    "~",
    ".crdownload",
    ".part",
    ".orig",
];

pub fn to_slashes(s: &str) -> String {
    if s.contains('\\') {
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
}

/// Windows can hand back verbatim paths (`\\?\C:\...`) and UNC verbatim paths
/// (`\\?\UNC\server\share`). Both are stripped before anything is stored.
pub fn strip_verbatim(s: &str) -> String {
    let s = to_slashes(s);
    if let Some(rest) = s.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else if let Some(rest) = s.strip_prefix("//?/") {
        rest.to_string()
    } else {
        s
    }
}

pub fn clean_path(p: &Path) -> PathBuf {
    PathBuf::from(strip_verbatim(&p.to_string_lossy()))
}

/// `\\server\share` cannot be watched reliably (and never should be: a network
/// share is not a repository).
pub fn is_unc(p: &Path) -> bool {
    let s = to_slashes(&p.to_string_lossy());
    s.starts_with("//") && !s.starts_with("//?/")
}

/// Repo-relative, forward-slash path. `None` when `abs` is outside `root`.
/// Comparison is ASCII-case-insensitive because NTFS is.
pub fn relative(root: &Path, abs: &Path) -> Option<String> {
    let r = to_slashes(&root.to_string_lossy());
    let a = to_slashes(&abs.to_string_lossy());
    let r = r.trim_end_matches('/');
    let a = a.trim_end_matches('/');

    if a.len() == r.len() && a.eq_ignore_ascii_case(r) {
        return Some(String::new());
    }
    let n = r.len();
    if a.len() > n && a.is_char_boundary(n) {
        let head = &a[..n];
        let rest = &a[n..];
        if head.eq_ignore_ascii_case(r) && (rest.starts_with('/') || rest.starts_with('\\')) {
            return Some(to_slashes(rest.trim_start_matches(['/', '\\'])));
        }
    }
    None
}

pub fn join_rel(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for part in rel.split('/') {
        if !part.is_empty() {
            p.push(part);
        }
    }
    p
}

pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn extension(path: &str) -> Option<&str> {
    let name = file_name(path);
    let dot = name.rfind('.')?;
    if dot == 0 {
        None
    } else {
        Some(&name[dot + 1..])
    }
}

/// Why a path is skipped. Returned so `watch --verbose` can say something more
/// useful than "ignored".
fn ignored_reason(rel: &str) -> Option<&'static str> {
    let lower = rel.to_ascii_lowercase();
    for seg in lower.split('/') {
        if IGNORED_SEGMENTS.contains(&seg) {
            return Some("excluded directory");
        }
    }
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if IGNORED_NAMES.contains(&name) {
        return Some("system file");
    }
    if IGNORED_SUFFIXES.iter().any(|s| name.ends_with(s)) {
        return Some("editor scratch file");
    }
    None
}

// ── glob matching ─────────────────────────────────────────────────────────

/// Shell-style glob: `*` stays inside one path segment, `**` crosses segments,
/// `?` is one non-separator byte. Case-sensitive; callers lower-case first when
/// the platform is.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    glob_bytes(pattern.as_bytes(), text.as_bytes())
}

fn glob_bytes(pattern: &[u8], text: &[u8]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    if pattern.starts_with(b"**") {
        let rest = &pattern[2..];
        if rest.first() == Some(&b'/') {
            // `**/` also matches zero directories.
            if glob_bytes(&rest[1..], text) {
                return true;
            }
            for index in 0..text.len() {
                if text[index] == b'/' && glob_bytes(&rest[1..], &text[index + 1..]) {
                    return true;
                }
            }
            return false;
        }
        if glob_bytes(rest, text) {
            return true;
        }
        for index in 0..text.len() {
            if glob_bytes(pattern, &text[index + 1..]) {
                return true;
            }
        }
        return false;
    }
    match pattern[0] {
        b'*' => {
            if glob_bytes(&pattern[1..], text) {
                return true;
            }
            !text.is_empty() && text[0] != b'/' && glob_bytes(pattern, &text[1..])
        }
        b'?' => !text.is_empty() && text[0] != b'/' && glob_bytes(&pattern[1..], &text[1..]),
        byte => !text.is_empty() && text[0] == byte && glob_bytes(&pattern[1..], &text[1..]),
    }
}

// ── .gitignore ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Rule {
    /// `!pattern` — re-includes a previously excluded path.
    negated: bool,
    /// Trailing `/`: the pattern only matches directories.
    directory_only: bool,
    /// Contains a `/`, so it is anchored at the repository root rather than
    /// matching at any depth.
    anchored: bool,
    pattern: String,
}

fn parse_gitignore(text: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches(['\r', ' ']);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (negated, rest) = match line.strip_prefix('!') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let (directory_only, rest) = match rest.strip_suffix('/') {
            Some(rest) => (true, rest),
            None => (false, rest),
        };
        // A leading `/` anchors the pattern at the repository root, with or
        // without a further `/`.
        let (leading_slash, rest) = match rest.strip_prefix('/') {
            Some(rest) => (true, rest),
            None => (false, rest),
        };
        if rest.is_empty() {
            continue;
        }
        let anchored = leading_slash || rest.contains('/');
        rules.push(Rule {
            negated,
            directory_only,
            anchored,
            pattern: rest.to_ascii_lowercase(),
        });
    }
    rules
}

fn rule_matches(rule: &Rule, lower_rel: &str) -> bool {
    let segments: Vec<&str> = lower_rel.split('/').collect();
    let directories = segments.len().saturating_sub(1);

    if rule.anchored {
        let candidates: &[&str] = if rule.directory_only {
            // Any ancestor directory, or the path itself when it is a directory.
            &segments[..=directories]
        } else {
            &segments[..]
        };
        if !rule.directory_only && glob_match(&rule.pattern, lower_rel) {
            return true;
        }
        for depth in 1..=candidates.len() {
            let prefix = candidates[..depth].join("/");
            if glob_match(&rule.pattern, &prefix) {
                return true;
            }
        }
        return false;
    }

    // Unanchored patterns match a single path segment at any depth. A directory
    // pattern excludes everything beneath it, so every segment is a candidate.
    segments
        .iter()
        .any(|segment| glob_match(&rule.pattern, segment))
}

/// The repository's ignore rules plus the built-in exclusions.
pub struct IgnoreSet {
    root: PathBuf,
    rules: Vec<Rule>,
}

impl IgnoreSet {
    pub fn new(root: &Path) -> Self {
        let rules = std::fs::read_to_string(root.join(".gitignore"))
            .map(|text| parse_gitignore(&text))
            .unwrap_or_default();
        IgnoreSet {
            root: root.to_path_buf(),
            rules,
        }
    }

    /// `true` when the path is outside the repo, or excluded by our list,
    /// by `.gitignore`, or by `.git/info/exclude`.
    pub fn is_ignored(&self, abs: &Path) -> bool {
        self.reason(abs).is_some()
    }

    pub fn reason(&self, abs: &Path) -> Option<&'static str> {
        let rel = match relative(&self.root, abs) {
            Some(r) => r,
            None => return Some("outside the watched repository"),
        };
        if rel.is_empty() {
            return Some("the repository root itself");
        }
        if let Some(why) = ignored_reason(&rel) {
            return Some(why);
        }
        // Later rules win, so a negation can rescue an earlier exclusion.
        let lower = rel.to_ascii_lowercase();
        let mut verdict = None;
        for rule in &self.rules {
            if rule_matches(rule, &lower) {
                verdict = Some(if rule.negated {
                    "re-included by .gitignore"
                } else {
                    ".gitignore"
                });
            }
        }
        match verdict {
            Some(".gitignore") => Some(".gitignore"),
            _ => None,
        }
    }
}

/// Test files, matched on shape rather than on a single ecosystem.
pub fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let segments: Vec<&str> = lower.split('/').collect();
    let directories = segments.len().saturating_sub(1);
    if segments[..directories].iter().any(|segment| {
        matches!(
            *segment,
            "test" | "tests" | "__tests__" | "__snapshots__" | "spec"
        )
    }) {
        return true;
    }

    let name = segments.last().copied().unwrap_or("");
    let stem_end = |suffix: &str| name.ends_with(suffix);
    if name.contains(".test.") || name.contains(".spec.") {
        return true;
    }
    if name.starts_with("test_") && (stem_end(".py") || stem_end(".rs") || stem_end(".rb")) {
        return true;
    }
    matches!(extension(name), Some("go" | "py" | "exs" | "rs")) && name.contains("_test.")
}

/// Dependency manifests we know how to read new package names out of.
pub fn dependency_file(path: &str) -> Option<&'static str> {
    let name = file_name(path).to_ascii_lowercase();
    Some(match name.as_str() {
        "package.json" => "npm",
        "cargo.toml" => "cargo",
        "requirements.txt" => "pip",
        "pyproject.toml" => "python",
        "go.mod" => "go",
        "gemfile" => "bundler",
        "pom.xml" => "maven",
        _ => return None,
    })
}

/// Files whose path alone is a secret disclosure.
pub fn is_secret_path(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    name.starts_with(".env")
        || name == ".npmrc"
        || name == ".pypirc"
        || name == ".netrc"
        || name.contains("credentials")
        || name.contains("id_rsa")
        || matches!(
            extension(path).map(|e| e.to_ascii_lowercase()).as_deref(),
            Some("pem") | Some("key") | Some("p12") | Some("pfx")
        )
}

pub fn os_name() -> &'static str {
    std::env::consts::OS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_verbatim_prefixes() {
        assert_eq!(
            strip_verbatim(r"\\?\C:\repo\src\main.rs"),
            "C:/repo/src/main.rs"
        );
        assert_eq!(
            strip_verbatim(r"\\?\UNC\srv\share\a.txt"),
            "//srv/share/a.txt"
        );
        assert_eq!(strip_verbatim("C:\\repo\\a.txt"), "C:/repo/a.txt");
    }

    #[test]
    fn relative_is_case_insensitive_on_windows_paths() {
        let root = Path::new("C:/Repo");
        assert_eq!(
            relative(root, Path::new("c:/repo/src/main.rs")).as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(relative(root, Path::new("C:/other/x.rs")), None);
        assert_eq!(relative(root, Path::new("C:/RepoX/x.rs")), None);
    }

    #[test]
    fn test_paths_are_recognised() {
        for p in [
            "src/foo.test.ts",
            "src/foo.spec.tsx",
            "tests/integration.rs",
            "__tests__/a.js",
            "pkg/test_thing.py",
            "pkg/thing_test.go",
        ] {
            assert!(is_test_path(p), "{p} should be a test path");
        }
        assert!(!is_test_path("src/testing_helpers.rs"));
        assert!(!is_test_path("src/latest.rs"));
        assert!(!is_test_path("src/contest/entry.rs"));
    }

    #[test]
    fn ignored_segments_and_gitignore() {
        let set = IgnoreSet::new(Path::new("C:/repo"));
        assert!(set.is_ignored(Path::new("C:/repo/node_modules/a/b.js")));
        assert!(set.is_ignored(Path::new("C:/repo/.flightstrip/objects/a")));
        assert!(!set.is_ignored(Path::new("C:/repo/src/main.rs")));
        assert!(set.is_ignored(Path::new("C:/elsewhere/main.rs")));
    }

    #[test]
    fn glob_star_does_not_cross_directories() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(!glob_match("*.rs", "src/main.rs"));
        assert!(glob_match("**/*.rs", "src/deep/main.rs"));
        assert!(glob_match("src/**/*.rs", "src/main.rs"));
        assert!(glob_match("?.toml", "a.toml"));
    }

    #[test]
    fn gitignore_rules_match_the_common_cases() {
        let rules = parse_gitignore(
            "# comment\n\
             /local-only.txt\n\
             logs/\n\
             *.log\n\
             !keep.log\n",
        );
        let matches = |rel: &str| {
            let lower = rel.to_ascii_lowercase();
            rules
                .iter()
                .any(|rule| rule_matches(rule, &lower) && !rule.negated)
        };
        assert!(matches("local-only.txt"));
        assert!(!matches("nested/local-only.txt"));
        assert!(matches("logs/today.txt"));
        assert!(matches("debug.log"));
        assert!(matches("a/b/debug.log"));
        assert!(!matches("src/main.rs"));
    }
}
