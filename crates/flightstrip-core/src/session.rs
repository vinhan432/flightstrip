//! The live session: an append-only `events.jsonl` plus a `record.json` sidecar
//! holding the wall-clock start.
//!
//! Event times are always relative to the session start (`t_ms`), so a clock
//! change mid-session cannot produce a negative gap. The sidecar carries the
//! absolute start, which is the only place it is needed.

use crate::json;
use crate::schema::{Event, EventKind, PromptSource, SessionLabel, SessionRecord, SessionStart};
use crate::time;
use crate::{io_error, paths, Error, Result};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const STATE_DIR: &str = ".flightstrip";
pub const EVENTS_FILE: &str = "events.jsonl";
pub const RECORD_FILE: &str = "record.json";

pub fn state_dir(root: &Path) -> PathBuf {
    root.join(STATE_DIR)
}

pub fn sessions_dir(root: &Path) -> PathBuf {
    state_dir(root).join("sessions")
}

pub fn active_path(root: &Path) -> PathBuf {
    state_dir(root).join("active")
}

pub fn session_dir(root: &Path, id: &str) -> PathBuf {
    sessions_dir(root).join(id)
}

pub fn events_path(dir: &Path) -> PathBuf {
    dir.join(EVENTS_FILE)
}

pub fn new_session_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    // A millisecond clock plus a per-process counter: two sessions started in
    // the same millisecond — `watch` restarting, say — still get distinct ids,
    // and no RNG crate is needed.
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    let now = time::now_ms();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = (now as u32)
        ^ std::process::id().wrapping_mul(0x9e37_79b9)
        ^ counter.wrapping_mul(0x85eb_ca6b);
    format!(
        "{}-{:04x}",
        time::format_stamp(now),
        (seed ^ (seed >> 16)) & 0xffff
    )
}

pub fn parse_started_at(id: &str) -> Option<u64> {
    time::parse_stamp(id.split('-').next()?)
}

pub fn repo_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "repository".to_string())
}

// ── active-session pointer ────────────────────────────────────────────────

pub struct ActiveRef {
    pub id: String,
    pub pid: u32,
}

pub fn read_active(root: &Path) -> Option<ActiveRef> {
    let path = active_path(root);
    let text = fs::read_to_string(&path).ok()?;
    let mut lines = text.lines();
    let id = lines.next()?.trim().to_string();
    if id.is_empty() {
        return None;
    }
    let pid = lines.next().and_then(|l| l.trim().parse::<u32>().ok());
    Some(ActiveRef {
        id,
        pid: pid.unwrap_or(0),
    })
}

pub fn write_active(root: &Path, id: &str) -> Result<()> {
    let path = active_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    let body = format!("{id}\n{}\n", std::process::id());
    crate::write_atomic(&path, body.as_bytes())
}

pub fn clear_active(root: &Path) {
    let _ = fs::remove_file(active_path(root));
}

/// Cheap liveness probe. `0` means "written by an older version": assume alive
/// rather than silently detaching from a running session.
pub fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(windows)]
    {
        // Opening the process with the barest rights is enough to prove it exists.
        use std::ffi::c_void;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
            fn CloseHandle(handle: *mut c_void) -> i32;
        }
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                false
            } else {
                CloseHandle(handle);
                true
            }
        }
    }
    #[cfg(not(windows))]
    {
        Path::new("/proc").join(pid.to_string()).exists()
            || std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
    }
}

// ── event log ─────────────────────────────────────────────────────────────

pub struct EventLog {
    dir: PathBuf,
    started_at_ms: u64,
    closed: bool,
}

impl EventLog {
    /// Start a new session on disk and write the opening event.
    pub fn create(
        root: &Path,
        title: &str,
        agent: Option<String>,
        label: SessionLabel,
    ) -> Result<EventLog> {
        let id = new_session_id();
        let dir = session_dir(root, &id);
        fs::create_dir_all(&dir).map_err(|e| io_error(&dir, e))?;
        let started_at_ms = time::now_ms();

        let record = SessionRecord {
            id: id.clone(),
            title: title.to_string(),
            agent: agent.clone(),
            started_at: time::format_rfc3339(started_at_ms),
            os: paths::os_name().to_string(),
            repo_name: repo_name(root),
            label,
        };
        let record_path = dir.join(RECORD_FILE);
        crate::write_atomic(
            &record_path,
            (json::to_pretty(&record.to_json()) + "\n").as_bytes(),
        )?;

        let mut log = EventLog {
            dir,
            started_at_ms,
            closed: false,
        };
        log.append(EventKind::SessionStart(SessionStart {
            title: title.to_string(),
            agent,
            os: paths::os_name().to_string(),
            repo_name: repo_name(root),
        }))?;
        Ok(log)
    }

    /// Re-open an existing session (used by `mark`, `exec` and `export`).
    pub fn open(root: &Path, id: &str) -> Result<EventLog> {
        let dir = session_dir(root, id);
        let record =
            read_record(&dir).ok_or_else(|| Error::Invalid(format!("no session called {id}")))?;
        let started_at_ms = time::parse_rfc3339(&record.started_at)
            .or_else(|| parse_started_at(id))
            .ok_or_else(|| Error::Invalid("session start time is unreadable".into()))?;
        Ok(EventLog {
            dir,
            started_at_ms,
            closed: false,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn id(&self) -> String {
        self.dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    pub fn elapsed_ms(&self) -> u64 {
        time::now_ms().saturating_sub(self.started_at_ms)
    }

    pub fn append(&mut self, kind: EventKind) -> Result<Event> {
        self.append_with(|_| kind)
    }

    /// Append under an exclusive lock, taking the sequence number from the log
    /// itself.
    ///
    /// `watch`, `mark` and `exec` are separate processes appending to one file,
    /// so an in-memory counter is not enough: two of them hand out the same
    /// number, and duplicate `seq` values make the exporter attach one file
    /// change's hunks to another.
    fn append_with<F>(&mut self, build: F) -> Result<Event>
    where
        F: FnOnce(u64) -> EventKind,
    {
        let path = events_path(&self.dir);
        let _lock = AppendLock::acquire(&self.dir)?;
        let seq = next_seq_on_disk(&self.dir)?;
        let event = Event::new(seq, self.elapsed_ms(), build(seq));
        let mut line = json::to_compact(&event.to_json());
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| io_error(&path, e))?;
        file.write_all(line.as_bytes())
            .map_err(|e| io_error(&path, e))?;
        file.flush().map_err(|e| io_error(&path, e))?;
        Ok(event)
    }

    /// Append a prompt. The id is derived from the sequence number, which makes
    /// it stable and readable in the report ("caused by prompt p-7").
    pub fn mark_prompt(
        &mut self,
        text: &str,
        source: PromptSource,
        agent: Option<String>,
    ) -> Result<Event> {
        let text = text.to_string();
        self.append_with(move |seq| {
            EventKind::Prompt(crate::schema::Prompt {
                id: format!("p-{seq}"),
                text,
                source,
                agent,
            })
        })
    }

    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        // Don't double-close a session that already ends properly.
        let events = self.events()?;
        if !matches!(events.last().map(|e| &e.kind), Some(EventKind::SessionEnd)) {
            self.append(EventKind::SessionEnd)?;
        }
        self.closed = true;
        Ok(())
    }

    pub fn events(&self) -> Result<Vec<Event>> {
        read_events(&self.dir)
    }
}

const LOCK_FILE: &str = "events.lock";

/// How long a writer waits for the append lock before giving up.
const LOCK_WAIT: Duration = Duration::from_secs(2);

/// A writer that dies while holding the lock leaves the file behind; after this
/// long it is treated as abandoned and the next writer takes it over.
const LOCK_STALE: Duration = Duration::from_secs(5);

/// One writer at a time may read the last sequence number and append, which is
/// what keeps `seq` unique across the recorder and the short-lived commands.
///
/// A real file lock would be released by the OS on a crash; a lock file is what
/// the standard library can express portably, and the staleness timeout covers
/// the difference.
struct AppendLock {
    path: PathBuf,
}

impl AppendLock {
    fn acquire(dir: &Path) -> Result<AppendLock> {
        let path = dir.join(LOCK_FILE);
        let deadline = Instant::now() + LOCK_WAIT;
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(AppendLock { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if is_stale(&path) {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    if Instant::now() >= deadline {
                        return Err(Error::Invalid(format!(
                            "the event log stayed locked for {}s; another recorder is probably stuck",
                            LOCK_WAIT.as_secs()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(io_error(&path, error)),
            }
        }
    }
}

impl Drop for AppendLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age > LOCK_STALE)
        .unwrap_or(false)
}

/// The next sequence number, read back from the log.
///
/// Reading the whole file per append is O(n), but `watch` already re-reads the
/// log every poll to find the current prompt, so this does not change the shape
/// of the cost.
fn next_seq_on_disk(dir: &Path) -> Result<u64> {
    Ok(read_events(dir)?
        .iter()
        .map(|event| event.seq)
        .max()
        .map(|seq| seq + 1)
        .unwrap_or(0))
}

pub fn read_record(dir: &Path) -> Option<SessionRecord> {
    let path = dir.join(RECORD_FILE);
    let text = fs::read_to_string(path).ok()?;
    let value = json::parse(&text).ok()?;
    SessionRecord::from_json(&value).ok()
}

pub fn count_events(dir: &Path) -> Result<u64> {
    Ok(read_events(dir)?.len() as u64)
}

/// Read the event log. Unparsable lines are skipped: a crash mid-write leaves a
/// torn last line, and losing one event is better than losing the recording.
pub fn read_events(dir: &Path) -> Result<Vec<Event>> {
    let path = events_path(dir);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let file = fs::File::open(&path).map_err(|e| io_error(&path, e))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = json::parse(&line) {
            if let Ok(event) = Event::from_json(&value) {
                events.push(event);
            }
        }
    }
    Ok(events)
}

/// The prompt that was most recently marked, whichever process marked it.
pub fn current_prompt(dir: &Path) -> Option<String> {
    let events = read_events(dir).ok()?;
    events.iter().rev().find_map(|e| match &e.kind {
        EventKind::Prompt(p) => Some(p.id.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_parse_back_to_a_time() {
        let id = new_session_id();
        assert!(parse_started_at(&id).is_some(), "{id}");
    }

    #[test]
    fn session_ids_are_unique_within_a_second() {
        let a = new_session_id();
        let b = new_session_id();
        assert_ne!(a, b);
    }

    #[test]
    fn event_log_round_trips() {
        let root = std::env::temp_dir().join(format!("fs-session-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut log = EventLog::create(&root, "test", None, SessionLabel::Recorded).unwrap();
        log.mark_prompt("do a thing", PromptSource::Mark, None)
            .unwrap();
        log.close().unwrap();
        let events = log.events().unwrap();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0].kind, EventKind::SessionStart(_)));
        assert!(matches!(events[1].kind, EventKind::Prompt(_)));
        assert!(matches!(events[2].kind, EventKind::SessionEnd));
        assert_eq!(events[0].seq, 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reopening_continues_the_sequence() {
        let root = std::env::temp_dir().join(format!("fs-session-reopen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut log = EventLog::create(&root, "test", None, SessionLabel::Recorded).unwrap();
        let id = log.id();
        log.mark_prompt("one", PromptSource::Mark, None).unwrap();
        drop(log);

        let mut log = EventLog::open(&root, &id).unwrap();
        let event = log
            .append(EventKind::Note(crate::schema::Note { text: "two".into() }))
            .unwrap();
        assert_eq!(event.seq, 2);
        assert_eq!(log.events().unwrap().len(), 3);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_lines_are_skipped_without_losing_the_recording() {
        let root = std::env::temp_dir().join(format!("fs-session-torn-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let mut log = EventLog::create(&root, "test", None, SessionLabel::Recorded).unwrap();
        log.mark_prompt("keep me", PromptSource::Mark, None)
            .unwrap();
        let dir = log.dir().to_path_buf();
        drop(log);

        let path = events_path(&dir);
        let mut body = fs::read_to_string(&path).unwrap();
        body.push_str("not json at all\n");
        body.push_str("{\"v\":1,\"seq\":99,\"t_ms\":1,\"kind\":\"unknown_kind\"}\n");
        body.push_str("{\"v\":1,\"seq\":100,\"t_ms\":2");
        fs::write(&path, body).unwrap();

        let events = read_events(&dir).unwrap();
        assert_eq!(events.len(), 2, "start and prompt survive: {events:#?}");
        assert!(matches!(events[1].kind, EventKind::Prompt(_)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn two_writers_never_share_a_sequence_number() {
        let root = std::env::temp_dir().join(format!("fs-session-lock-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let log = EventLog::create(&root, "test", None, SessionLabel::Recorded).unwrap();
        let id = log.id();
        drop(log);

        // `mark` and `exec` open the log while `watch` still holds a handle: both
        // processes must get their own number.
        let mut first = EventLog::open(&root, &id).unwrap();
        let mut second = EventLog::open(&root, &id).unwrap();
        let a = first
            .append(EventKind::Note(crate::schema::Note { text: "a".into() }))
            .unwrap();
        let b = second
            .append(EventKind::Note(crate::schema::Note { text: "b".into() }))
            .unwrap();
        assert_ne!(a.seq, b.seq);
        assert_eq!(b.seq, a.seq + 1);

        let seqs: Vec<u64> = first.events().unwrap().iter().map(|e| e.seq).collect();
        assert_eq!(seqs, vec![0, 1, 2]);
        let _ = fs::remove_dir_all(&root);
    }
}
