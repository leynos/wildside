//! Session key fingerprinting for operational visibility.
//!
//! Provides a truncated SHA-256 fingerprint of the session signing key,
//! enabling operators to verify which key is active without exposing the key
//! material itself. Fingerprints are logged on startup and referenced in
//! rotation runbooks.

use actix_web::cookie::Key;
use sha2::{Digest, Sha256};

/// Length of the fingerprint in bytes before hex encoding.
const FINGERPRINT_BYTES: usize = 8;

/// Generate a truncated SHA-256 fingerprint of the key's signing material.
///
/// Returns the first 8 bytes of the SHA-256 hash as a 16-character hex string.
/// This is sufficient for visual distinction in logs and runbooks without
/// being security-sensitive.
///
/// # Examples
///
/// ```rust
/// use actix_web::cookie::Key;
/// use backend::inbound::http::session_config::fingerprint::key_fingerprint;
///
/// let key = Key::generate();
/// let fp = key_fingerprint(&key);
///
/// assert_eq!(fp.len(), 16);
/// assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
/// ```
#[must_use]
pub fn key_fingerprint(key: &Key) -> String {
    let signing_bytes = key.signing();
    let mut hasher = Sha256::new();
    hasher.update(signing_bytes);
    let result = hasher.finalize();
    hex::encode(&result[..FINGERPRINT_BYTES])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn fingerprint_is_deterministic() {
        // Same key should produce same fingerprint
        let key_bytes = vec![b'a'; 64];
        let key = Key::derive_from(&key_bytes);

        let fp1 = key_fingerprint(&key);
        let fp2 = key_fingerprint(&key);

        assert_eq!(fp1, fp2, "fingerprint should be deterministic");
    }

    #[rstest]
    fn fingerprint_has_correct_length() {
        let key = Key::generate();
        let fp = key_fingerprint(&key);

        assert_eq!(
            fp.len(),
            FINGERPRINT_BYTES * 2,
            "fingerprint should be 16 hex characters"
        );
    }

    #[rstest]
    fn fingerprint_is_valid_hex() {
        let key = Key::generate();
        let fp = key_fingerprint(&key);

        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint should only contain hex characters"
        );
    }

    #[rstest]
    fn different_keys_produce_different_fingerprints() {
        let key1 = Key::derive_from(&[b'a'; 64]);
        let key2 = Key::derive_from(&[b'b'; 64]);

        let fp1 = key_fingerprint(&key1);
        let fp2 = key_fingerprint(&key2);

        assert_ne!(
            fp1, fp2,
            "different keys should have different fingerprints"
        );
    }

    #[rstest]
    fn fingerprint_is_lowercase_hex() {
        let key = Key::generate();
        let fp = key_fingerprint(&key);

        assert_eq!(fp, fp.to_lowercase(), "fingerprint should be lowercase hex");
    }
}
