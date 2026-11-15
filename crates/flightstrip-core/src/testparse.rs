//! Best-effort test output parsing.
//!
//! This is guesswork over human-facing output, and the README says so. Nothing
//! here is allowed to fail loudly: unknown output means "no test run detected",
//! not an error.
//!
//! Written against the handful of summary shapes that actually appear in agent
//! transcripts, rather than a regex engine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedTests {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub total: u32,
    pub ok: bool,
}

/// Terminal colour codes defeat every pattern below, so they go first.
pub fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut index = 0usize;
    while index < bytes.len() {
        // ESC [ params final-byte
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'[') {
            let mut cursor = index + 2;
            while cursor < bytes.len() {
                let byte = bytes[cursor];
                cursor += 1;
                if (0x40..=0x7e).contains(&byte) {
                    break;
                }
            }
            index = cursor;
            continue;
        }
        let ch_len = match bytes[index] {
            0x00..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            _ => 4,
        };
        out.push_str(&s[index..(index + ch_len).min(s.len())]);
        index += ch_len;
    }
    out
}

pub fn looks_like_test_command(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    [
        "test",
        "spec",
        "pytest",
        "jest",
        "vitest",
        "mocha",
        "rspec",
        "phpunit",
        "ctest",
        "cargo nextest",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// `4 passed` — the number immediately before a recognised word. The last one
/// wins, because summaries repeat earlier output (jest prints per-suite lines
/// before the total).
fn last_count_before(text: &str, words: &[&str]) -> Option<u32> {
    let mut found = None;
    let mut previous: Option<&str> = None;
    for raw in text.split_whitespace() {
        let word = raw
            .trim_matches(|c: char| {
                c == ',' || c == '.' || c == ';' || c == ':' || c == ')' || c == '('
            })
            .to_ascii_lowercase();
        if words.contains(&word.as_str()) {
            if let Some(count) = previous.and_then(|p| p.parse::<u32>().ok()) {
                found = Some(count);
            }
        }
        previous = Some(raw);
    }
    found
}

/// `test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out`.
/// Counted once per test binary, so several lines are summed.
fn parse_cargo(text: &str) -> Option<(u32, u32, u32, bool)> {
    let mut hits = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let mut all_ok = true;

    let mut rest = text;
    while let Some(position) = rest.find("test result:") {
        let tail = &rest[position..];
        let line = tail.split('\n').next().unwrap_or(tail);
        hits += 1;
        all_ok &= line.contains("test result: ok");
        passed += last_count_before(line, &["passed"]).unwrap_or(0);
        failed += last_count_before(line, &["failed"]).unwrap_or(0);
        skipped += last_count_before(line, &["ignored"]).unwrap_or(0);
        rest = &tail[line.len()..];
    }
    if hits == 0 {
        return None;
    }
    Some((passed, failed, skipped, all_ok))
}

/// TAP and node's own reporter end a run with counted summary lines:
///
/// ```text
/// # tests 3        ℹ tests 4
/// # pass 2         ℹ pass 2
/// # fail 1         ℹ fail 2
/// # skipped 0      ℹ skipped 0
/// ```
///
/// Read as `marker word count`. Both the pass and the fail line must be present,
/// so output that merely starts with `#` cannot invent a test run.
fn parse_tap(text: &str) -> Option<(u32, u32, u32)> {
    let mut passed = None;
    let mut failed = None;
    let mut skipped = 0u32;

    for line in text.lines() {
        let trimmed = line.trim();
        let rest = match trimmed
            .strip_prefix("# ")
            .or_else(|| trimmed.strip_prefix("ℹ "))
        {
            Some(rest) => rest,
            None => continue,
        };
        let mut parts = rest.split_whitespace();
        let (Some(word), Some(count)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(count) = count.parse::<u32>() else {
            continue;
        };
        match word {
            "pass" => passed = Some(count),
            "fail" => failed = Some(count),
            "skipped" => skipped = count,
            _ => {}
        }
    }

    Some((passed?, failed?, skipped))
}

fn parse_go(text: &str) -> Option<(u32, u32)> {
    let mut passed = 0u32;
    let mut failed = 0u32;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--- PASS") {
            passed += 1;
        } else if trimmed.starts_with("--- FAIL") {
            failed += 1;
        }
    }
    if passed + failed == 0 {
        None
    } else {
        Some((passed, failed))
    }
}

/// Parse a captured stdout/stderr blob. `exit_code` is the tiebreaker when the
/// output says nothing useful.
pub fn parse(cmd: &str, output: &str, exit_code: Option<i32>) -> Option<ParsedTests> {
    let text = strip_ansi(output);
    let exit_ok = exit_code.unwrap_or(0) == 0;

    if let Some((passed, failed, skipped, all_ok)) = parse_cargo(&text) {
        return Some(ParsedTests {
            passed,
            failed,
            skipped,
            total: passed + failed + skipped,
            ok: failed == 0 && all_ok && exit_ok,
        });
    }

    if let Some((passed, failed, skipped)) = parse_tap(&text) {
        return Some(ParsedTests {
            passed,
            failed,
            skipped,
            total: passed + failed + skipped,
            ok: failed == 0 && exit_ok,
        });
    }

    if let Some((passed, failed)) = parse_go(&text) {
        return Some(ParsedTests {
            passed,
            failed,
            skipped: 0,
            total: passed + failed,
            ok: failed == 0 && exit_ok,
        });
    }

    // pytest / jest / vitest / mocha / rspec all print "N passed" style summaries.
    // mocha spells it "3 passing".
    let passed = last_count_before(&text, &["passed", "passing"]);
    let failed = last_count_before(&text, &["failed", "failing"]);
    let skipped = last_count_before(&text, &["skipped", "pending", "ignored"]);
    if let Some(passed) = passed {
        let failed = failed.unwrap_or(0);
        let skipped = skipped.unwrap_or(0);
        return Some(ParsedTests {
            passed,
            failed,
            skipped,
            total: passed + failed + skipped,
            ok: failed == 0 && exit_ok,
        });
    }
    if let Some(failed) = failed {
        if failed > 0 {
            return Some(ParsedTests {
                passed: 0,
                failed,
                skipped: skipped.unwrap_or(0),
                total: failed + skipped.unwrap_or(0),
                ok: false,
            });
        }
    }

    // Nothing recognisable: trust the exit code, and admit the run is opaque by
    // reporting a single anonymous test.
    if !looks_like_test_command(cmd) {
        return None;
    }
    let ok = exit_code.unwrap_or(0) == 0;
    Some(ParsedTests {
        passed: u32::from(ok),
        failed: u32::from(!ok),
        skipped: 0,
        total: 1,
        ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_sequences() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(strip_ansi("\x1b[1;32m ok \x1b[0m"), " ok ");
    }

    #[test]
    fn parses_cargo_summary() {
        let out =
            "running 3 tests\n...\ntest result: ok. 3 passed; 0 failed; 1 ignored; 0 measured\n";
        let p = parse("cargo test", out, Some(0)).unwrap();
        assert_eq!((p.passed, p.failed, p.skipped, p.total), (3, 0, 1, 4));
        assert!(p.ok);
    }

    #[test]
    fn sums_multiple_cargo_binaries() {
        let out = "test result: ok. 2 passed; 0 failed; 0 ignored\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n";
        let p = parse("cargo test", out, Some(101)).unwrap();
        assert_eq!((p.passed, p.failed), (2, 1));
        assert!(!p.ok);
    }

    #[test]
    fn parses_pytest_summary() {
        let out = "==== 2 failed, 5 passed, 1 skipped in 0.42s ====";
        let p = parse("pytest", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.skipped), (5, 2, 1));
        assert!(!p.ok);
    }

    #[test]
    fn parses_jest_summary_with_ansi() {
        let out = "\x1b[31mTests:\x1b[0m \x1b[1m1 failed\x1b[0m, \x1b[1m4 passed\x1b[0m, 5 total";
        let p = parse("npm test", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed), (4, 1));
    }

    #[test]
    fn parses_go_verbose() {
        let out =
            "--- PASS: TestA (0.00s)\n--- PASS: TestB (0.00s)\n--- FAIL: TestC (0.00s)\nFAIL\n";
        let p = parse("go test ./...", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.total), (2, 1, 3));
    }

    #[test]
    fn falls_back_to_exit_code() {
        let p = parse("cargo test", "weird output", Some(0)).unwrap();
        assert_eq!((p.passed, p.total), (1, 1));
        assert!(p.ok);
    }

    #[test]
    fn ignores_non_test_commands() {
        assert!(parse("ls -la", "total 12", Some(0)).is_none());
    }

    // The cases below are trimmed copies of real runs, kept because these are the
    // shapes that actually show up in transcripts and that a summary-only test
    // would miss.

    #[test]
    fn parses_real_pytest_quiet_output() {
        let out = r#"
>       assert 1 + 1 == 3, "math is broken"
E       AssertionError: math is broken
=========================== short test summary info ===========================
FAILED tests/test_sample.py::test_bad - AssertionError: math is broken
1 failed, 2 passed, 1 skipped in 0.15s
"#;
        let p = parse("python -m pytest -q", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.skipped, p.total), (2, 1, 1, 4));
        assert!(!p.ok);
    }

    #[test]
    fn real_pytest_verbose_per_test_lines_do_not_beat_the_summary() {
        let out = r#"
tests/test_sample.py::test_ok PASSED                                     [ 25%]
tests/test_sample.py::test_ok2 PASSED                                    [ 50%]
tests/test_sample.py::test_bad FAILED                                    [ 75%]
FAILED tests/test_sample.py::test_bad - AssertionError: math is broken
=================== 1 failed, 2 passed, 1 skipped in 0.18s ====================
"#;
        let p = parse("python -m pytest -v", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.skipped, p.total), (2, 1, 1, 4));
        assert!(!p.ok);
    }

    #[test]
    fn parses_real_jest_output() {
        let out = r#"
  ● add › broken case

    expect(received).toBe(expected) // Object.is equality

    Expected: 5
    Received: 4

Test Suites: 1 failed, 1 total
Tests:       1 failed, 3 passed, 4 total
Snapshots:   0 total
Time:        1.79 s
"#;
        let p = parse("npm test", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.total), (3, 1, 4));
        assert!(!p.ok);

        // A green run: the "Test Suites" line must not be read as a test count.
        let green = "Test Suites: 1 passed, 1 total\nTests:       4 passed, 4 total\n";
        let p = parse("jest", green, Some(0)).unwrap();
        assert_eq!((p.passed, p.failed, p.total), (4, 0, 4));
        assert!(p.ok);
    }

    #[test]
    fn parses_real_cargo_failure() {
        let out = "\nrunning 3 tests\n\nfailures:\n    tests::broken\n\ntest result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\nerror: test failed, to rerun pass `--lib`\n";
        let p = parse("cargo test", out, Some(101)).unwrap();
        assert_eq!((p.passed, p.failed, p.total), (2, 1, 3));
        assert!(!p.ok);
    }

    #[test]
    fn parses_node_test_tap_summary() {
        let out = r#"TAP version 13
# Subtest: adds
ok 1 - adds
# Subtest: broken
not ok 3 - broken
1..3
# tests 3
# suites 0
# pass 2
# fail 1
# cancelled 0
# skipped 0
# todo 0
# duration_ms 378.4241
"#;
        let p = parse("node --test --test-reporter=tap", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.skipped, p.total), (2, 1, 0, 3));
        assert!(!p.ok);
    }

    #[test]
    fn parses_node_test_default_reporter_summary() {
        let out = "ℹ tests 4\nℹ suites 0\nℹ pass 2\nℹ fail 2\nℹ cancelled 0\nℹ skipped 1\nℹ todo 0\nℹ duration_ms 267.9593\n";
        let p = parse("node --test js/", out, Some(1)).unwrap();
        assert_eq!((p.passed, p.failed, p.skipped, p.total), (2, 2, 1, 5));
        assert!(!p.ok);
    }

    #[test]
    fn tap_summary_without_a_fail_line_is_not_a_test_run() {
        // Only half a summary is not enough to call it a run.
        assert!(parse("cat notes.md", "# pass 2\n", Some(0)).is_none());
    }
}
