//! `.flightstrip/config.toml`.
//!
//! A deliberately small key/value format: the settings are five scalars and one
//! string array, so a full TOML dependency would be more code than the parser.
//! Unknown keys are errors rather than silently ignored, because a typo in
//! `thrash_threshold` otherwise looks configured and never takes effect.

use crate::error::{Context, Result};
use flightstrip_core::paths;
use flightstrip_core::session;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub ignore_globs: Vec<String>,
    pub poll_ms: u64,
    pub thrash_threshold: u32,
    pub attribution_decay_minutes: u64,
    pub redact_by_default: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ignore_globs: Vec::new(),
            poll_ms: 500,
            thrash_threshold: flightstrip_core::detectors::thrashing::THRESHOLD,
            attribution_decay_minutes: 5,
            redact_by_default: false,
        }
    }
}

impl Config {
    pub fn path(root: &Path) -> PathBuf {
        session::state_dir(root).join(FILE_NAME)
    }

    pub fn exists(root: &Path) -> bool {
        Self::path(root).is_file()
    }

    /// Read the config, using defaults when the file is absent.
    pub fn load(root: &Path) -> Result<Config> {
        let path = Self::path(root);
        if !path.is_file() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("while reading {}", path.display()))?;
        Config::parse(&text)
            .with_context(|| format!("{} is not a valid Flightstrip config", path.display()))
    }

    pub fn parse(text: &str) -> Result<Config> {
        let mut config = Config::default();
        for (index, raw) in text.lines().enumerate() {
            let line_no = index + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {line_no}: expected `key = value`, found `{line}`"))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "ignore_globs" => config.ignore_globs = parse_string_array(value, line_no)?,
                "poll_ms" => config.poll_ms = parse_uint(value, line_no, key)?,
                "thrash_threshold" => {
                    let threshold = parse_uint(value, line_no, key)?;
                    config.thrash_threshold = u32::try_from(threshold).map_err(|_| {
                        format!("line {line_no}: `thrash_threshold` must fit in a u32")
                    })?;
                }
                "attribution_decay_minutes" => {
                    config.attribution_decay_minutes = parse_uint(value, line_no, key)?
                }
                "redact_by_default" => {
                    config.redact_by_default = match value {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(format!(
                        "line {line_no}: `redact_by_default` expects true or false, not `{other}`"
                    ))
                        }
                    }
                }
                other => {
                    return Err(format!(
                        "line {line_no}: unknown key `{other}`; known keys are ignore_globs, \
poll_ms, thrash_threshold, attribution_decay_minutes, redact_by_default"
                    ))
                }
            }
        }
        Ok(config)
    }

    pub fn render(&self) -> String {
        let globs = self
            .ignore_globs
            .iter()
            .map(|glob| format!("\"{glob}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "# Flightstrip project configuration.\n\
# Regenerate with: flightstrip init --force\n\
\n\
# Extra repo-relative globs to keep out of the recording (besides .gitignore).\n\
ignore_globs = [{globs}]\n\
\n\
# How often the working tree is polled, in milliseconds.\n\
poll_ms = {}\n\
\n\
# Changes to one file inside one prompt before `thrashing` fires.\n\
thrash_threshold = {}\n\
\n\
# Minutes after a prompt with no command before a change is unattributed.\n\
attribution_decay_minutes = {}\n\
\n\
# Make `export` and `report` redact secret-looking values unless told otherwise.\n\
redact_by_default = {}\n",
            self.poll_ms,
            self.thrash_threshold,
            self.attribution_decay_minutes,
            self.redact_by_default
        )
    }

    /// Write the config, returning `true` when a new file was created.
    pub fn write(root: &Path, force: bool) -> Result<(PathBuf, bool)> {
        let path = Self::path(root);
        if path.is_file() && !force {
            return Ok((path, false));
        }
        flightstrip_core::write_atomic(&path, Config::default().render().as_bytes())
            .with_context(|| format!("while writing {}", path.display()))?;
        Ok((path, true))
    }
}

fn parse_uint(value: &str, line: usize, key: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|_| format!("line {line}: `{key}` expects a non-negative number, not `{value}`"))
}

fn unquote(value: &str, line: usize) -> Result<String> {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        return Ok(value[1..value.len() - 1].to_string());
    }
    Err(format!(
        "line {line}: expected a quoted string, found `{value}`"
    ))
}

fn parse_string_array(value: &str, line: usize) -> Result<Vec<String>> {
    let value = value.trim();
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| {
            format!("line {line}: expected an array like [\"*.log\"], found `{value}`")
        })?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in inner.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && quote == Some('"') {
            escaped = true;
            continue;
        }
        match quote {
            Some(active) => {
                current.push(ch);
                if ch == active {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' => {
                    current.push(ch);
                    quote = Some(ch);
                }
                ',' => {
                    let item = current.trim();
                    if !item.is_empty() {
                        items.push(unquote(item, line)?);
                    }
                    current.clear();
                }
                _ => current.push(ch),
            },
        }
    }
    if quote.is_some() {
        return Err(format!("line {line}: unterminated string in array"));
    }
    let item = current.trim();
    if !item.is_empty() {
        items.push(unquote(item, line)?);
    }
    Ok(items)
}

/// Whether a `.gitignore` line ignores the state directory. All four forms a
/// person or editor actually writes count: `.flightstrip`, `.flightstrip/`,
/// `/.flightstrip` and `/.flightstrip/`. The root-anchored pair matters because
/// a hand-written `.gitignore` usually anchors its entries.
pub fn ignores_state_dir(line: &str) -> bool {
    let line = line.trim();
    line.trim_start_matches('/').trim_end_matches('/') == ".flightstrip"
}

/// Add `.flightstrip/` to an existing root `.gitignore`. Returns `true` when the
/// file was changed. A missing `.gitignore` is left alone: creating one is a
/// repository decision, not a recorder one.
pub fn ensure_gitignore(root: &Path) -> Result<bool> {
    let path = root.join(".gitignore");
    if !path.is_file() {
        return Ok(false);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("while reading {}", path.display()))?;
    if text.lines().any(ignores_state_dir) {
        return Ok(false);
    }
    let mut updated = text;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(".flightstrip/\n");
    flightstrip_core::write_atomic(&path, updated.as_bytes())
        .with_context(|| format!("while writing {}", path.display()))?;
    Ok(true)
}

/// A path for display in doctor/status output.
pub fn display_path(path: &Path) -> String {
    paths::to_slashes(&path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_render() {
        let config = Config::default();
        assert_eq!(Config::parse(&config.render()).unwrap(), config);
    }

    #[test]
    fn arrays_and_comments_are_read() {
        let config = Config::parse(
            "# comment\nignore_globs = [\"*.log\", \"tmp/**\"]\npoll_ms = 900\nredact_by_default = true\n",
        )
        .unwrap();
        assert_eq!(config.ignore_globs, vec!["*.log", "tmp/**"]);
        assert_eq!(config.poll_ms, 900);
        assert!(config.redact_by_default);
    }

    #[test]
    fn a_typo_is_an_error_not_a_default() {
        let error = Config::parse("thrash_treshold = 9\n").unwrap_err();
        assert!(error.to_string().contains("unknown key"), "{error}");
    }

    #[test]
    fn malformed_values_name_the_line() {
        let error = Config::parse("poll_ms = fast\n").unwrap_err();
        assert!(error.to_string().contains("line 1"), "{error}");
    }

    #[test]
    fn all_four_gitignore_forms_count_as_ignored() {
        for line in [
            ".flightstrip",
            ".flightstrip/",
            "/.flightstrip",
            "/.flightstrip/",
            "  /.flightstrip  ",
        ] {
            assert!(ignores_state_dir(line), "{line}");
        }
        for line in [
            "target/",
            "node_modules",
            ".flightstrip.bak",
            "dist/.flightstrip",
        ] {
            assert!(!ignores_state_dir(line), "{line}");
        }
    }

    #[test]
    fn a_root_anchored_entry_is_not_duplicated() {
        let root = std::env::temp_dir().join(format!("fs-config-anchored-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".gitignore"), "/target\n/.flightstrip\n").unwrap();
        assert!(!ensure_gitignore(&root).unwrap());
        assert_eq!(
            std::fs::read_to_string(root.join(".gitignore")).unwrap(),
            "/target\n/.flightstrip\n",
            "the file is left untouched"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn gitignore_is_only_touched_when_it_already_exists() {
        let root = std::env::temp_dir().join(format!("fs-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        assert!(!ensure_gitignore(&root).unwrap());
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        assert!(ensure_gitignore(&root).unwrap());
        assert!(std::fs::read_to_string(root.join(".gitignore"))
            .unwrap()
            .contains(".flightstrip/"));
        assert!(
            !ensure_gitignore(&root).unwrap(),
            "adding it twice is a no-op"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
