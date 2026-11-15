//! Content hashing for the snapshot store.
//!
//! 128-bit FNV-1a. This is a *dedupe key*, not a security primitive: it decides
//! whether two snapshots are the same bytes and names the object file. FNV-1a is
//! four lines of arithmetic, which is why it replaces `blake3` here.
//!
//! Collision risk at this scale is negligible: a repository produces thousands of
//! hashes, not billions, and a collision would only mean one snapshot is reused
//! for another — visible immediately as a wrong diff, never as a wrong verdict.

const OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

/// Lower-case hex, 32 characters: the object's name inside the store.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= *byte as u128;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:032x}")
}

/// Guard for anything that becomes a file path inside the store.
pub fn is_hash(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_stable_hex() {
        let hash = content_hash(b"hello\n");
        assert!(is_hash(&hash), "{hash}");
        assert_eq!(hash, content_hash(b"hello\n"));
    }

    #[test]
    fn different_bytes_hash_differently() {
        assert_ne!(content_hash(b"a\n"), content_hash(b"b\n"));
        // A single flipped byte in a long file must be noticed.
        let mut long = vec![b'x'; 4096];
        let before = content_hash(&long);
        long[2048] = b'y';
        assert_ne!(before, content_hash(&long));
    }

    #[test]
    fn path_traversal_is_not_a_hash() {
        assert!(!is_hash("../../etc/passwd"));
        assert!(!is_hash(""));
    }
}
