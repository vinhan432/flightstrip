//! Diff generation between two stored snapshots.
//!
//! Myers' O(ND) diff, implemented here rather than pulled in. The recording only
//! ever diffs one file against its previous state, so D (the number of inserted
//! plus deleted lines) is usually tiny — a handful of edits — and the algorithm
//! is at its best.
//!
//! The edit budget is capped. A file rewritten wholesale is reported as "the
//! whole file changed" instead of spending seconds and megabytes finding a
//! minimal script, because `session.json` truncates the result anyway.

use crate::schema::{DiffHunk, DiffLine, DiffTag};

/// Total insertions + deletions we are willing to search for. Above this, the
/// change is reported as a whole-file replacement.
const MAX_EDITS: usize = 512;

#[derive(Debug, Clone)]
pub struct DiffOptions {
    pub max_hunks: usize,
    pub max_lines_per_hunk: usize,
    pub max_line_chars: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        DiffOptions {
            max_hunks: 3,
            max_lines_per_hunk: 20,
            max_line_chars: 200,
        }
    }
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let line = line.trim_end_matches(['\n', '\r']);
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    let mut out: String = line.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn split_lines(bytes: &[u8]) -> Vec<&str> {
    let text = std::str::from_utf8(bytes).unwrap_or("");
    // A trailing newline is a terminator, not an extra empty line.
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n').collect()
}

/// Matched `(old_index, new_index)` pairs, in order. `None` when the edit budget
/// is exhausted.
fn matching_lines(old: &[&str], new: &[&str]) -> Option<Vec<(usize, usize)>> {
    let n = old.len() as isize;
    let m = new.len() as isize;
    if n == 0 || m == 0 {
        return Some(Vec::new());
    }
    let limit = MAX_EDITS.min((n + m) as usize);
    let width = 2 * limit + 3;
    let offset = limit as isize + 1;

    // `v[k + offset]` is the furthest x reached on diagonal k. We keep the vector
    // after every depth so the path can be walked back.
    let mut v = vec![0isize; width];
    let mut trace: Vec<Vec<isize>> = Vec::new();

    for d in 0..=limit as isize {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let index = (k + offset) as usize;
            let mut x = if k == -d || (k != d && v[index - 1] < v[index + 1]) {
                v[index + 1]
            } else {
                v[index - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && old[x as usize] == new[y as usize] {
                x += 1;
                y += 1;
            }
            v[index] = x;
            if x >= n && y >= m {
                return Some(backtrack(&trace, offset, n, m));
            }
            k += 2;
        }
    }
    None
}

fn at(trace: &[Vec<isize>], depth: usize, offset: isize, k: isize) -> isize {
    let entry = &trace[depth];
    let index = k + offset;
    if index < 0 || index as usize >= entry.len() {
        0
    } else {
        entry[index as usize]
    }
}

fn backtrack(trace: &[Vec<isize>], offset: isize, n: isize, m: isize) -> Vec<(usize, usize)> {
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let mut x = n;
    let mut y = m;
    let last_depth = trace.len() - 1;

    for depth in (0..=last_depth).rev() {
        let d = depth as isize;
        let k = x - y;
        let (prev_k, prev_x) = if depth == 0 {
            (0, 0)
        } else {
            // `trace[depth]` is the vector as it was *before* depth `depth`,
            // which is the state left behind by depth `depth - 1`.
            let down = at(trace, depth, offset, k + 1);
            let right = at(trace, depth, offset, k - 1);
            if k == -d || (k != d && right < down) {
                (k + 1, down)
            } else {
                (k - 1, right)
            }
        };
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            matches.push((x as usize, y as usize));
        }
        x = prev_x;
        y = prev_y;
    }

    matches.reverse();
    matches
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    Equal,
    Add,
    Remove,
}

/// The full change list. `(tag, old_line_number, new_line_number, text)`.
fn changes<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(Tag, usize, usize, &'a str)> {
    let matches = match matching_lines(old, new) {
        Some(matches) => matches,
        None => {
            // Over budget: report a replacement rather than a misleading diff.
            let mut out = Vec::with_capacity(old.len() + new.len());
            for (index, line) in old.iter().enumerate() {
                out.push((Tag::Remove, index + 1, 0, *line));
            }
            for (index, line) in new.iter().enumerate() {
                out.push((Tag::Add, 0, index + 1, *line));
            }
            return out;
        }
    };

    let mut out = Vec::new();
    let mut old_index = 0usize;
    let mut new_index = 0usize;
    for (match_old, match_new) in matches {
        while old_index < match_old {
            out.push((Tag::Remove, old_index + 1, 0, old[old_index]));
            old_index += 1;
        }
        while new_index < match_new {
            out.push((Tag::Add, 0, new_index + 1, new[new_index]));
            new_index += 1;
        }
        out.push((Tag::Equal, old_index + 1, new_index + 1, old[old_index]));
        old_index += 1;
        new_index += 1;
    }
    while old_index < old.len() {
        out.push((Tag::Remove, old_index + 1, 0, old[old_index]));
        old_index += 1;
    }
    while new_index < new.len() {
        out.push((Tag::Add, 0, new_index + 1, new[new_index]));
        new_index += 1;
    }
    out
}

/// Added and removed line counts. Used for `FileChange` bookkeeping.
pub fn line_stats(old: &[u8], new: &[u8]) -> (u32, u32) {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let mut added = 0u32;
    let mut removed = 0u32;
    for (tag, _, _, _) in changes(&old_lines, &new_lines) {
        match tag {
            Tag::Add => added += 1,
            Tag::Remove => removed += 1,
            Tag::Equal => {}
        }
    }
    (added, removed)
}

/// Build the display hunks. Two context lines are kept so a removed assertion is
/// readable next to the code around it.
pub fn hunks(old: &[u8], new: &[u8], opts: &DiffOptions) -> Vec<DiffHunk> {
    if old == new {
        return Vec::new();
    }
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let all = changes(&old_lines, &new_lines);

    // Group: a run of more than 2*context equal lines separates two hunks.
    let context = 2usize;
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut last_change = 0usize;
    let mut seen_change = false;
    let mut equal_run = 0usize;

    for (index, (tag, _, _, _)) in all.iter().enumerate() {
        if *tag == Tag::Equal {
            if seen_change {
                equal_run += 1;
                if equal_run > context * 2 {
                    groups.push((start, last_change + context));
                    seen_change = false;
                    equal_run = 0;
                }
            }
        } else {
            if !seen_change {
                start = index.saturating_sub(context);
                seen_change = true;
            }
            last_change = index;
            equal_run = 0;
        }
    }
    if seen_change {
        groups.push((start, (last_change + context).min(all.len() - 1)));
    }

    let mut out = Vec::new();
    for (group_index, (from, to)) in groups.iter().enumerate() {
        if group_index >= opts.max_hunks {
            break;
        }
        let slice = &all[*from..=*to];
        let old_start = slice
            .iter()
            .find(|(tag, _, _, _)| *tag != Tag::Add)
            .map(|(_, old_line, _, _)| *old_line)
            .unwrap_or(1) as u32;
        let new_start = slice
            .iter()
            .find(|(tag, _, _, _)| *tag != Tag::Remove)
            .map(|(_, _, new_line, _)| *new_line)
            .unwrap_or(1) as u32;

        let mut lines = Vec::new();
        let mut cut = false;
        for (tag, _, _, text) in slice {
            if lines.len() >= opts.max_lines_per_hunk {
                cut = true;
                break;
            }
            lines.push(DiffLine {
                tag: match tag {
                    Tag::Equal => DiffTag::Context,
                    Tag::Add => DiffTag::Add,
                    Tag::Remove => DiffTag::Remove,
                },
                text: truncate_line(text, opts.max_line_chars),
            });
        }
        if cut {
            lines.push(DiffLine {
                tag: DiffTag::Context,
                text: "… (hunk truncated)".to_string(),
            });
        }
        out.push(DiffHunk {
            old_start,
            new_start,
            lines,
        });
    }

    if groups.len() > opts.max_hunks {
        out.push(DiffHunk {
            old_start: 0,
            new_start: 0,
            lines: vec![DiffLine {
                tag: DiffTag::Context,
                text: format!("… ({} more hunks truncated)", groups.len() - opts.max_hunks),
            }],
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(hunks: &[DiffHunk]) -> Vec<(DiffTag, String)> {
        hunks
            .iter()
            .flat_map(|hunk| hunk.lines.iter())
            .map(|line| (line.tag, line.text.clone()))
            .collect()
    }

    #[test]
    fn counts_added_and_removed() {
        let (added, removed) = line_stats(b"a\nb\nc\n", b"a\nB\nc\nd\n");
        assert_eq!((added, removed), (2, 1));
    }

    #[test]
    fn identical_content_has_no_hunks() {
        assert!(hunks(b"same\n", b"same\n", &DiffOptions::default()).is_empty());
    }

    #[test]
    fn one_changed_line_is_located_precisely() {
        let old = b"one\ntwo\nthree\nfour\nfive\n";
        let new = b"one\ntwo\nTHREE\nfour\nfive\n";
        let hunks = hunks(old, new, &DiffOptions::default());
        assert_eq!(hunks.len(), 1);
        let lines = flat(&hunks);
        assert!(lines.contains(&(DiffTag::Remove, "three".to_string())));
        assert!(lines.contains(&(DiffTag::Add, "THREE".to_string())));
        // Context lines survive so the hunk is readable on its own...
        assert!(lines.contains(&(DiffTag::Context, "two".to_string())));
        assert!(lines.contains(&(DiffTag::Context, "four".to_string())));
        // ...but only two on each side, not the whole file.
        assert!(lines.len() <= 6, "{} lines in one hunk", lines.len());
    }

    #[test]
    fn hunk_carries_the_removed_assertion() {
        let old = b"test('x', () => {\n  expect(add(1, 2)).toBe(3);\n});\n";
        let new = b"test('x', () => {\n  expect(add(1, 2)).toBeTruthy();\n});\n";
        let text: Vec<String> = flat(&hunks(old, new, &DiffOptions::default()))
            .into_iter()
            .map(|(_, text)| text)
            .collect();
        assert!(text.iter().any(|line| line.contains("toBe(3)")));
    }

    #[test]
    fn long_lines_are_capped() {
        let long = "x".repeat(500);
        let old = format!("{long}\n");
        let new = "y\n".to_string();
        for (_, text) in flat(&hunks(
            old.as_bytes(),
            new.as_bytes(),
            &DiffOptions::default(),
        )) {
            assert!(text.chars().count() <= 200, "line not capped: {text}");
        }
    }

    #[test]
    fn hunk_count_is_capped() {
        let mut old = String::new();
        let mut new = String::new();
        for i in 0..40 {
            old.push_str(&format!("line {i}\n"));
            if i % 5 == 0 {
                new.push_str(&format!("changed {i}\n"));
            } else {
                new.push_str(&format!("line {i}\n"));
            }
        }
        let hunks = hunks(old.as_bytes(), new.as_bytes(), &DiffOptions::default());
        assert!(
            hunks.len() <= 4,
            "expected 3 hunks plus a truncation marker"
        );
    }

    #[test]
    fn creates_and_deletes_diff_against_empty() {
        let created = hunks(b"", b"a\nb\n", &DiffOptions::default());
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].lines.len(), 2);
        assert!(created[0].lines.iter().all(|line| line.tag == DiffTag::Add));

        let deleted = hunks(b"a\nb\n", b"", &DiffOptions::default());
        assert!(deleted[0]
            .lines
            .iter()
            .all(|line| line.tag == DiffTag::Remove));
    }

    #[test]
    fn swapping_lines_is_not_a_whole_file_rewrite() {
        let old = b"a\nb\nc\nd\n";
        let new = b"d\nc\nb\na\n";
        let (added, removed) = line_stats(old, new);
        assert!(added + removed <= 8, "{added} added, {removed} removed");
    }

    #[test]
    fn a_rewrite_falls_back_without_hanging() {
        let old: String = (0..2000).map(|i| format!("old {i}\n")).collect();
        let new: String = (0..2000).map(|i| format!("new {i}\n")).collect();
        let hunks = hunks(old.as_bytes(), new.as_bytes(), &DiffOptions::default());
        assert!(!hunks.is_empty());
        let (added, removed) = line_stats(old.as_bytes(), new.as_bytes());
        assert_eq!((added, removed), (2000, 2000));
    }
}
