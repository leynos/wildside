//! Compile-fail fixture: streaming into a `sha2` hasher via `io::copy` must not
//! compile.
//!
//! `sha2` 0.11 dropped the `std::io::Write` implementation on its hashers, so
//! bytes must be fed through `Digest::update` from a bounded read loop instead.
//! If this fixture ever compiles again — most likely because `sha2` was
//! downgraded to 0.10 — the migration has regressed and the surrounding
//! trybuild test fails.
//!
//! The sink is passed through a local generic wrapper rather than handed to
//! `io::copy` directly. Both fail with the same missing-bound error, but the
//! direct form also makes rustc quote `std::io::copy`'s signature, and that
//! snippet is emitted only when the standard library sources are installed.
//! CI installs a minimal toolchain without `rust-src`, so the direct form
//! would pin an expectation that depends on the host's `rust-src` rather than
//! on the bound this fixture exists to assert.

use std::io;

use sha2::{Digest, Sha256};

/// The pre-migration shape: feeding a hasher through a `W: io::Write` sink.
fn stream_into<W: io::Write>(sink: &mut W) -> io::Result<u64> {
    io::copy(&mut io::Cursor::new(b"wildside"), sink)
}

fn main() {
    let mut hasher = Sha256::new();
    let _ = stream_into(&mut hasher);
}
