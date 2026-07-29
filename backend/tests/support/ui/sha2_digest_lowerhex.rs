//! Compile-fail fixture: `{:x}` formatting of a `sha2` digest must not compile.
//!
//! `sha2` 0.11 finalizes to `hybrid_array::Array<u8, _>`, which does not
//! implement `core::fmt::LowerHex`. Digests are therefore encoded explicitly
//! with `hex::encode`. If this fixture ever compiles again — most likely
//! because `sha2` was downgraded to 0.10 — the migration has regressed and the
//! surrounding trybuild test fails.

use sha2::{Digest, Sha256};

fn main() {
    let _ = format!("{:x}", Sha256::digest(b"wildside"));
}
