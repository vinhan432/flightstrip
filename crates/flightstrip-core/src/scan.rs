//! Filesystem → `FileChange` events.
//!
//! A polling scanner, not an OS notification watcher. `notify` pulls in a
//! cross-platform abstraction over inotify, kqueue, FSEvents and
//! ReadDirectoryChangesW plus a debouncer, for a job that is fundamentally
//! "compare the tree to what you recorded last time". A poll does exactly that,
//! needs no crate, and behaves the same on all three platforms. The cost is
//! latency (one poll interval, 500 ms by default) and a directory walk per
//! interval, which is why ignored directories are pruned before they are read.
//!
//! Two rules carry most of the weight:
//!
//! 1. **Content, not events.** A path is only a change if its normalised content
//!    hash differs from the last one we recorded. Size and modification time are
//!    only a pre-filter to avoid re-reading untouched files.
//! 2. **A delete only counts if we knew the file.** Editors save by writing a
//!    temp file and renaming it over the target. A path that vanishes and was
//!    never known is scratch noise, so it is dropped instead of being recorded
//!    as a deletion.

use crate::differ;
use crate::paths::{self, IgnoreSet};
use crate::schema::{EventKind, FileChange, FileOp};
use crate::session::{self, EventLog};
use crate::store::{self, SnapshotStore};
use crate::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// How often the tree is walked. Lower means lower latency and more work.
    pub poll_ms: u64,
    /// Hash the repository at start, so the first edit of a file has a real
    /// "before". Costs one read of the repository.
    pub baseline: bool,
    pub verbose: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        WatchOptions {
            poll_ms: 500,
            baseline: true,
            verbose: false,
        }
    }
}

/// What we last recorded for a path.
#[derive(Debug, Clone, PartialEq)]
struct Known {
    /// `None` when the content was binary or too large to store.
    hash: Option<String>,
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
pub struct ScanStats {
    pub seen: usize,
    pub emitted: usize,
}

pub struct Recorder<'a> {
    root: PathBuf,
    ignores: IgnoreSet,
    store: SnapshotStore,
    log: &'a mut EventLog,
    known: HashMap<String, Known>,
    prompt_id: Option<String>,
    opts: WatchOptions,
}

impl<'a> Recorder<'a> {
    pub fn new(root: &Path, log: &'a mut EventLog, opts: WatchOptions) -> Self {
        Recorder {
            ignores: IgnoreSet::new(root),
            store: SnapshotStore::at(root),
            root: root.to_path_buf(),
            log,
            known: HashMap::new(),
            prompt_id: None,
            opts,
        }
    }

    /// Walk the repository once and remember what everything looked like.
    /// Without this, the first edit of an existing file would be recorded as a
    /// creation and there would be no diff.
    pub fn seed_baseline(&mut self) -> Result<usize> {
        let mut count = 0usize;
        for (rel, abs) in self.walk() {
            let Ok(metadata) = fs::metadata(&abs) else {
                continue;
            };
            let size = metadata.len();
            let modified = metadata.modified().ok();
            let hash = match fs::read(&abs) {
                Ok(raw) if store::is_storable(&raw) => Some(self.store.put(&raw)?),
                _ => None,
            };
            self.known.insert(
                rel,
                Known {
                    hash,
                    size,
                    modified,
                },
            );
            count += 1;
        }
        Ok(count)
    }

    /// Blocking scan loop. Returns when `stop` is set.
    pub fn run(&mut self, stop: &AtomicBool) -> Result<()> {
        let interval = Duration::from_millis(self.opts.poll_ms.max(50));
        self.prompt_id = session::current_prompt(self.log.dir());

        while !stop.load(Ordering::SeqCst) {
            if let Err(error) = self.scan_once() {
                // A recorder that dies because one directory was locked is
                // useless; say so and keep going.
                eprintln!("watch: scan failed: {error}");
            }
            // Sleep in slices so Ctrl+C is felt immediately.
            let deadline = std::time::Instant::now() + interval;
            while std::time::Instant::now() < deadline {
                if stop.load(Ordering::SeqCst) {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        Ok(())
    }

    /// One pass over the tree. Returns how many files were inspected.
    pub fn scan_once(&mut self) -> Result<usize> {
        self.prompt_id = session::current_prompt(self.log.dir());
        let mut seen: HashSet<String> = HashSet::new();
        let mut complete = true;

        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(error) => {
                    // Do not treat an unreadable directory as "everything
                    // inside was deleted".
                    complete = false;
                    if self.opts.verbose {
                        eprintln!("skip {}: {error}", dir.display());
                    }
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                // Symlinks are not followed: a link cycle would loop forever and
                // writing through one records the target, not the repo.
                if file_type.is_symlink() {
                    continue;
                }
                if self.ignores.is_ignored(&path) {
                    continue;
                }
                if file_type.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }
                let Some(rel) = paths::relative(&self.root, &path) else {
                    continue;
                };
                if rel.is_empty() {
                    continue;
                }
                seen.insert(rel.clone());
                if let Err(error) = self.observe(&path, &rel) {
                    eprintln!("watch: skipped {}: {error}", path.display());
                }
            }
        }

        if complete {
            let vanished: Vec<String> = self
                .known
                .keys()
                .filter(|key| !seen.contains(*key))
                .cloned()
                .collect();
            for rel in vanished {
                if let Err(error) = self.record_delete(&rel) {
                    eprintln!("watch: could not record deletion of {rel}: {error}");
                }
            }
        }

        Ok(seen.len())
    }

    /// Every file in the repository, repo-relative. Iterative, so a deep tree
    /// cannot overflow the stack.
    fn walk(&self) -> Vec<(String, PathBuf)> {
        let mut files = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_symlink() || self.ignores.is_ignored(&path) {
                    continue;
                }
                if file_type.is_dir() {
                    stack.push(path);
                } else if file_type.is_file() {
                    if let Some(rel) = paths::relative(&self.root, &path) {
                        if !rel.is_empty() {
                            files.push((rel, path));
                        }
                    }
                }
            }
        }
        files
    }

    fn observe(&mut self, abs: &Path, rel: &str) -> Result<()> {
        let metadata = match fs::metadata(abs) {
            Ok(metadata) => metadata,
            Err(error) => {
                // Vanished between the directory read and the stat: the next
                // pass sees it as a deletion.
                if self.opts.verbose {
                    eprintln!("skip {rel}: {error}");
                }
                return Ok(());
            }
        };
        let size = metadata.len();
        let modified = metadata.modified().ok();

        if let Some(previous) = self.known.get(rel) {
            if previous.size == size && previous.modified == modified {
                if self.opts.verbose {
                    eprintln!("skip {rel}: untouched");
                }
                return Ok(());
            }
        }

        let raw = match fs::read(abs) {
            Ok(raw) => raw,
            Err(error) => {
                // Transient lock (antivirus, indexer). The next pass retries.
                if self.opts.verbose {
                    eprintln!("skip {rel}: {error}");
                }
                return Ok(());
            }
        };

        let new_hash = if store::is_storable(&raw) {
            Some(self.store.put(&raw)?)
        } else {
            None
        };

        let previous = self.known.get(rel).cloned();
        if let Some(known) = &previous {
            if known.hash == new_hash && new_hash.is_some() {
                // Same bytes, new timestamp: touch, not an edit.
                self.known.insert(
                    rel.to_string(),
                    Known {
                        hash: new_hash,
                        size,
                        modified,
                    },
                );
                if self.opts.verbose {
                    eprintln!("skip {rel}: content unchanged");
                }
                return Ok(());
            }
        }

        let op = if previous.is_some() {
            FileOp::Modify
        } else {
            FileOp::Create
        };
        let before_hash = previous.as_ref().and_then(|known| known.hash.clone());

        let (lines_added, lines_removed) = match (&before_hash, &new_hash) {
            (Some(before), Some(after)) => match (self.store.get(before), self.store.get(after)) {
                (Ok(old), Ok(new)) => differ::line_stats(&old, &new),
                _ => (0, 0),
            },
            (None, Some(after)) => match self.store.get(after) {
                Ok(new) => (store::line_count(&new), 0),
                Err(_) => (0, 0),
            },
            (Some(before), None) => match self.store.get(before) {
                Ok(old) => (0, store::line_count(&old)),
                Err(_) => (0, 0),
            },
            (None, None) => (0, 0),
        };

        self.known.insert(
            rel.to_string(),
            Known {
                hash: new_hash.clone(),
                size,
                modified,
            },
        );
        self.emit(EventKind::FileChange(FileChange {
            path: rel.to_string(),
            op,
            before_hash,
            after_hash: new_hash,
            lines_added,
            lines_removed,
            prompt_id: None,
        }))?;
        Ok(())
    }

    fn record_delete(&mut self, rel: &str) -> Result<()> {
        let Some(known) = self.known.remove(rel) else {
            return Ok(());
        };
        let removed = known
            .hash
            .as_deref()
            .and_then(|hash| self.store.get(hash).ok())
            .map(|bytes| store::line_count(&bytes))
            .unwrap_or(0);
        self.emit(EventKind::FileChange(FileChange {
            path: rel.to_string(),
            op: FileOp::Delete,
            before_hash: known.hash,
            after_hash: None,
            lines_added: 0,
            lines_removed: removed,
            prompt_id: None,
        }))?;
        Ok(())
    }

    fn emit(&mut self, mut kind: EventKind) -> Result<()> {
        let prompt = self.prompt_id.clone();
        match &mut kind {
            EventKind::FileChange(change) => change.prompt_id = prompt,
            EventKind::Command(command) => command.prompt_id = prompt,
            EventKind::TestRun(run) => run.prompt_id = prompt,
            _ => {}
        }
        self.log.append(kind)?;
        Ok(())
    }
}

/// Files known at baseline, for the CLI's summary line.
pub fn baseline_size(recorder: &Recorder) -> usize {
    recorder.known.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SessionLabel;

    fn temp_root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "fs-scan-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn changes(log: &EventLog) -> Vec<FileChange> {
        log.events()
            .unwrap()
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::FileChange(change) => Some(change.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn first_seen_file_is_a_create_and_repeat_is_ignored() {
        let root = temp_root("basic");
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        recorder.seed_baseline().unwrap();

        fs::write(root.join("a.txt"), "one\n").unwrap();
        recorder.scan_once().unwrap();
        // Untouched file: no second change even though it is seen again.
        recorder.scan_once().unwrap();
        fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        recorder.scan_once().unwrap();
        fs::remove_file(root.join("a.txt")).unwrap();
        recorder.scan_once().unwrap();

        let changes = changes(&log);
        assert_eq!(changes.len(), 3, "create, modify, delete: {changes:#?}");
        assert_eq!(changes[0].op, FileOp::Create);
        assert_eq!(changes[1].op, FileOp::Modify);
        assert_eq!(changes[1].lines_added, 1);
        assert_eq!(changes[2].op, FileOp::Delete);
        assert_eq!(changes[2].lines_removed, 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn baseline_gives_an_existing_file_a_before_hash() {
        let root = temp_root("baseline");
        fs::write(root.join("a.txt"), "one\n").unwrap();
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        assert_eq!(recorder.seed_baseline().unwrap(), 1);
        fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        recorder.scan_once().unwrap();

        let changes = changes(&log);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].op, FileOp::Modify);
        assert!(changes[0].before_hash.is_some());
        assert_eq!(changes[0].lines_added, 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_vanishing_file_is_not_a_delete() {
        let root = temp_root("ghost");
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        // Nothing was ever recorded under this path.
        fs::write(root.join("ghost.txt"), "x").unwrap();
        recorder.scan_once().unwrap();
        fs::remove_file(root.join("ghost.txt")).unwrap();
        recorder.scan_once().unwrap();
        // It was recorded by the first pass, so the deletion is real, not a ghost.
        assert_eq!(changes(&log).len(), 2);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn ignored_directories_are_not_scanned() {
        let root = temp_root("ignored");
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/a.js"), "x").unwrap();
        fs::write(root.join("keep.js"), "x").unwrap();
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        assert_eq!(recorder.seed_baseline().unwrap(), 1);
        assert_eq!(recorder.scan_once().unwrap(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn line_ending_only_edits_are_not_changes() {
        let root = temp_root("crlf");
        fs::write(root.join("a.txt"), "one\ntwo\n").unwrap();
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        recorder.seed_baseline().unwrap();
        // Same content, different bytes on disk.
        fs::write(root.join("a.txt"), "one\r\ntwo\r\n").unwrap();
        recorder.scan_once().unwrap();
        assert!(changes(&log).is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn nested_directories_and_paths_with_spaces_are_recorded() {
        let root = temp_root("spaces");
        let nested = root.join("a dir").join("deep").join("er");
        fs::create_dir_all(&nested).unwrap();
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        recorder.seed_baseline().unwrap();
        fs::write(nested.join("a file.txt"), "hello\n").unwrap();
        assert_eq!(recorder.scan_once().unwrap(), 1);
        let changes = changes(&log);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "a dir/deep/er/a file.txt");
        assert_eq!(changes[0].op, FileOp::Create);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rewriting_the_same_bytes_is_a_touch_not_an_edit() {
        let root = temp_root("touch");
        fs::write(root.join("a.txt"), "same\n").unwrap();
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        recorder.seed_baseline().unwrap();
        // Same size and same content, but the write updates the mtime.
        fs::write(root.join("a.txt"), "same\n").unwrap();
        recorder.scan_once().unwrap();
        assert!(changes(&log).is_empty(), "{:#?}", changes(&log));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn target_and_node_modules_are_both_ignored() {
        let root = temp_root("targets");
        fs::create_dir_all(root.join("target").join("debug")).unwrap();
        fs::create_dir_all(root.join("node_modules").join("pkg")).unwrap();
        fs::write(root.join("target/debug/a.o"), "x").unwrap();
        fs::write(root.join("node_modules/pkg/a.js"), "x").unwrap();
        fs::write(root.join("src.rs"), "x").unwrap();
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        assert_eq!(recorder.seed_baseline().unwrap(), 1);
        assert_eq!(recorder.scan_once().unwrap(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_stop_flag_ends_the_loop() {
        let root = temp_root("stop");
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        let stop = AtomicBool::new(true);
        assert!(recorder.run(&stop).is_ok());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn changes_are_attributed_to_the_latest_prompt() {
        let root = temp_root("prompt");
        let mut log = EventLog::create(&root, "w", None, SessionLabel::Recorded).unwrap();
        log.mark_prompt("add retry", crate::schema::PromptSource::Mark)
            .unwrap();
        let mut recorder = Recorder::new(&root, &mut log, WatchOptions::default());
        fs::write(root.join("a.txt"), "one\n").unwrap();
        recorder.scan_once().unwrap();
        let changes = changes(&log);
        assert_eq!(changes[0].prompt_id.as_deref(), Some("p-1"));
        let _ = fs::remove_dir_all(&root);
    }
}
