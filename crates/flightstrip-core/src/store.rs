//! Content-addressed snapshot store.
//!
//! Layout: `.flightstrip/objects/<first two hex chars>/<hash>.snap`. The shard
//! directory keeps NTFS directories small (it degrades past ~10k entries).
//!
//! Snapshots are stored as plain LF-normalised bytes. There is no compressor:
//! an LZ4 or zstd crate costs build time and a C toolchain, and the store is
//! disposable (`rm -rf .flightstrip`). The tradeoff is disk, and NOTES.md says so.
//!
//! Objects are never deleted automatically — that is deliberate.

use crate::hash::{content_hash, is_hash};
use crate::paths;
use crate::{io_error, Error, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Files above this size are recorded as opaque changes (hash + line counts
/// only). Keeping a 200 MB build artifact out of the store matters more than
/// diffing it.
pub const MAX_CONTENT_BYTES: usize = 1024 * 1024;

/// Bytes examined when looking for the NUL that marks a binary file.
const BINARY_PROBE_BYTES: usize = 8192;

/// LF-normalise. CRLF and lone CR both collapse to LF so that a checkout
/// difference, or an editor's line-ending setting, cannot manufacture a change.
pub fn normalize_line_endings(raw: &[u8]) -> Vec<u8> {
    if !raw.contains(&b'\r') {
        return raw.to_vec();
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        let byte = raw[index];
        if byte == b'\r' {
            out.push(b'\n');
            if index + 1 < raw.len() && raw[index + 1] == b'\n' {
                index += 1;
            }
        } else {
            out.push(byte);
        }
        index += 1;
    }
    out
}

pub fn is_binary(raw: &[u8]) -> bool {
    raw.iter().take(BINARY_PROBE_BYTES).any(|byte| *byte == 0)
}

/// Whether the full content should be stored at all.
pub fn is_storable(raw: &[u8]) -> bool {
    raw.len() <= MAX_CONTENT_BYTES && !is_binary(raw)
}

pub fn line_count(normalized: &[u8]) -> u32 {
    if normalized.is_empty() {
        return 0;
    }
    let newlines = normalized.iter().filter(|byte| **byte == b'\n').count() as u32;
    if normalized.last() == Some(&b'\n') {
        newlines
    } else {
        newlines + 1
    }
}

pub struct SnapshotStore {
    objects: PathBuf,
}

impl SnapshotStore {
    pub fn at(repo_root: &Path) -> Self {
        SnapshotStore {
            objects: repo_root.join(".flightstrip").join("objects"),
        }
    }

    pub fn objects_dir(&self) -> &Path {
        &self.objects
    }

    fn object_path(&self, hash: &str) -> Result<PathBuf> {
        if !is_hash(hash) {
            return Err(Error::Invalid(format!("not a content hash: {hash}")));
        }
        Ok(self.objects.join(&hash[0..2]).join(format!("{hash}.snap")))
    }

    pub fn has(&self, hash: &str) -> bool {
        self.object_path(hash)
            .map(|path| path.is_file())
            .unwrap_or(false)
    }

    /// Store LF-normalised content and return its hash. Deduplicates and writes
    /// atomically (temp file in the same directory, then rename) so a crash
    /// mid-write cannot leave a torn object behind.
    pub fn put(&self, raw: &[u8]) -> Result<String> {
        let normalized = normalize_line_endings(raw);
        let hash = content_hash(&normalized);
        let destination = self.object_path(&hash)?;
        if destination.is_file() {
            return Ok(hash);
        }
        let directory = destination
            .parent()
            .ok_or_else(|| Error::Invalid("object path has no parent".into()))?;
        fs::create_dir_all(directory).map_err(|error| io_error(directory, error))?;

        let temporary = directory.join(format!(".{}.{}.tmp", &hash[..16], std::process::id()));
        {
            let mut file =
                fs::File::create(&temporary).map_err(|error| io_error(&temporary, error))?;
            file.write_all(&normalized)
                .map_err(|error| io_error(&temporary, error))?;
            file.flush().map_err(|error| io_error(&temporary, error))?;
        }
        match fs::rename(&temporary, &destination) {
            Ok(()) => Ok(hash),
            Err(_) if destination.is_file() => {
                // Someone else stored the same object first. Not a problem.
                let _ = fs::remove_file(&temporary);
                Ok(hash)
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(io_error(&destination, error))
            }
        }
    }

    /// Read LF-normalised content back. Used when building diffs.
    pub fn get(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.object_path(hash)?;
        fs::read(&path).map_err(|error| io_error(&path, error))
    }

    /// Content for a hash that may be missing (a truncated or pruned store must
    /// not fail an export).
    pub fn get_opt(&self, hash: &Option<String>) -> Option<Vec<u8>> {
        hash.as_ref().and_then(|hash| self.get(hash).ok())
    }
}

/// Deterministic key for case-insensitive path comparison.
pub fn dedup_key(rel: &str) -> String {
    paths::to_slashes(rel).to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_and_lf_hash_identically() {
        let a = normalize_line_endings(b"a\r\nb\r\n");
        let b = normalize_line_endings(b"a\nb\n");
        assert_eq!(a, b);
        assert_eq!(content_hash(&a), content_hash(&b));
    }

    #[test]
    fn lone_cr_is_normalised() {
        assert_eq!(normalize_line_endings(b"a\rb"), b"a\nb".to_vec());
    }

    #[test]
    fn line_counts_ignore_trailing_newline() {
        assert_eq!(line_count(b""), 0);
        assert_eq!(line_count(b"a"), 1);
        assert_eq!(line_count(b"a\n"), 1);
        assert_eq!(line_count(b"a\nb"), 2);
    }

    #[test]
    fn binary_detection() {
        assert!(is_binary(b"\x00\x01\x02"));
        assert!(!is_binary(b"hello"));
        assert!(!is_storable(&vec![b'a'; MAX_CONTENT_BYTES + 1]));
    }

    #[test]
    fn store_round_trips_and_dedups() {
        let directory = std::env::temp_dir().join(format!("fs-store-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let store = SnapshotStore::at(&directory);
        let first = store.put(b"one\r\ntwo\r\n").unwrap();
        let second = store.put(b"one\ntwo\n").unwrap();
        assert_eq!(first, second);
        assert_eq!(store.get(&first).unwrap(), b"one\ntwo\n".to_vec());
        assert!(store.has(&first));
        // The object is smaller than a directory listing of shards, not a tree.
        assert!(store.objects_dir().join(&first[0..2]).is_dir());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn rejects_path_traversal_in_hash() {
        let store = SnapshotStore::at(Path::new("."));
        assert!(store.get("../../etc/passwd").is_err());
    }
}
