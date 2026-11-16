//! Flightstrip core: the recorder behind the strip chart.
//!
//! The module split mirrors the job: [`schema`] defines the wire format,
//! [`json`] reads and writes it without a serializer crate, [`session`] owns the
//! live event log, [`store`] keeps content-addressed snapshots, [`scan`] polls
//! the working tree and turns changes into events, [`differ`] turns two snapshots
//! into hunks, [`detectors`] are pure functions over the event log, and [`export`]
//! is the only thing that writes `session.json`.
//!
//! There are no third-party dependencies on purpose (see NOTES.md). Everything
//! here is std-only: the JSON codec, the timestamp formatter, the 128-bit
//! content hash, the diff, the glob matcher, and the polling scanner.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub mod attributor;
pub mod detectors;
pub mod differ;
pub mod export;
pub mod hash;
pub mod json;
pub mod paths;
pub mod redact;
pub mod report;
pub mod scan;
pub mod schema;
pub mod session;
pub mod store;
pub mod testparse;
pub mod time;

pub use schema::*;

/// Errors raised by the core. The CLI prints them with context.
#[derive(Debug)]
pub enum Error {
    Io {
        path: String,
        source: std::io::Error,
    },
    Json(String),
    Invalid(String),
    Msg(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io { path, source } => write!(f, "i/o error while handling {path}: {source}"),
            Error::Json(message) => write!(f, "malformed JSON: {message}"),
            Error::Invalid(message) => write!(f, "invalid session: {message}"),
            Error::Msg(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Attach the path an I/O error happened at. Paths are almost always the useful
/// part of a recorder failure, so they never get dropped.
pub fn io_error(path: impl AsRef<Path>, source: std::io::Error) -> Error {
    Error::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}

/// Write a file through a temporary in the same directory, then rename.
///
/// `session.json`, `record.json` and the active-session pointer are rewritten
/// while another process may be reading them. A plain truncate-and-write leaves
/// a window where a reader sees half a document; the temporary keeps the old
/// file intact until the replacement is complete. The temporary name includes
/// the pid and a counter so two writers cannot collide.
pub fn write_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = path.as_ref();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let temporary = parent.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&temporary, bytes).map_err(|e| io_error(&temporary, e))?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        // On some Windows filesystems rename refuses an existing destination.
        // Removing first is only a fallback; the normal path is atomic.
        Err(_) if path.exists() => {
            std::fs::remove_file(path).map_err(|e| io_error(path, e))?;
            std::fs::rename(&temporary, path).map_err(|e| {
                let _ = std::fs::remove_file(&temporary);
                io_error(path, e)
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(io_error(path, error))
        }
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_an_existing_file() {
        let root = std::env::temp_dir().join(format!("fs-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("session.json");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            std::fs::read_dir(&root).unwrap().flatten().count(),
            1,
            "no temporary files are left behind"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
