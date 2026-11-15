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
