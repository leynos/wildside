//! Compile-fail fixture: streaming into a `sha2` hasher via `io::copy` must not
//! compile.
//!
//! `sha2` 0.11 dropped the `std::io::Write` implementation on its hashers, so
//! bytes must be fed through `Digest::update` from a bounded read loop instead.
//! If this fixture ever compiles again — most likely because `sha2` was
//! downgraded to 0.10 — the migration has regressed and the surrounding
//! trybuild test fails.

use std::io;

use sha2::{Digest, Sha256};

fn main() {
    let mut hasher = Sha256::new();
    let mut reader = io::Cursor::new(b"wildside");
    let _ = io::copy(&mut reader, &mut hasher);
}
