//! ULID-shaped identifiers for manually seeded Apalis rows.
//!
//! Apalis parses manually seeded job IDs as ULIDs. The fixtures need their
//! syntax and uniqueness, but do not depend on ULID time ordering, so a
//! UUID-backed Crockford Base32 encoder avoids a direct `ulid` dependency.

use uuid::Uuid;

const CROCKFORD_BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Generate a unique ULID-shaped identifier using the existing UUID crate.
///
/// # Examples
///
/// ```text
/// let id = apalis_job_id();
/// assert_eq!(id.len(), 26);
/// ```
pub fn apalis_job_id() -> String {
    let mut value = Uuid::new_v4().as_u128();
    let mut encoded = [b'0'; 26];

    for digit in encoded.iter_mut().rev() {
        *digit = CROCKFORD_BASE32[(value & 0b1_1111) as usize];
        value >>= 5;
    }

    String::from_utf8(encoded.to_vec()).expect("Crockford Base32 contains valid UTF-8")
}

#[cfg(test)]
mod tests {
    //! Verify fixture identifiers retain the ULID syntax Apalis expects.

    use super::{CROCKFORD_BASE32, apalis_job_id};

    #[test]
    fn generated_apalis_job_ids_have_ulid_syntax() {
        let id = apalis_job_id();

        assert_eq!(id.len(), 26);
        assert!(id.as_bytes()[0] <= b'7');
        assert!(id.bytes().all(|digit| CROCKFORD_BASE32.contains(&digit)));
    }
}
