//! Idempotency primitives for safe request retries.
//!
//! This module provides types for implementing idempotent request handling:
//!
//! - [`IdempotencyKey`]: Validated UUID identifier sent by clients via the
//!   `Idempotency-Key` HTTP header.
//! - [`PayloadHash`]: SHA-256 hash of a canonicalized request payload, used to
//!   detect conflicting requests for the same key.
//! - [`IdempotencyRecord`]: Stored record linking a key to its payload hash and
//!   original response.
//! - [`IdempotencyLookupResult`]: Outcome of looking up an idempotency key in
//!   the store.
//! - [`MutationType`]: Discriminator for different outbox-backed operations.
//! - [`IdempotencyConfig`]: Configuration for idempotency TTL.
//!
//! # Payload Canonicalization
//!
//! To ensure semantically equivalent payloads produce identical hashes
//! regardless of whitespace or key ordering, payloads are canonicalized before
//! hashing:
//!
//! 1. JSON objects have their keys sorted recursively.
//! 2. The result is serialized to compact JSON (no whitespace).
//! 3. The SHA-256 hash is computed on the resulting bytes.

mod config;
mod key;
mod mutation_type;
mod payload;
mod record;

pub use config::{
    DefaultIdempotencyEnv, IDEMPOTENCY_TTL_HOURS_ENV, IdempotencyConfig, IdempotencyEnv,
};
pub use key::{IdempotencyKey, IdempotencyKeyValidationError};
pub use mutation_type::{MutationType, ParseMutationTypeError};
pub use payload::{PayloadHash, PayloadHashError, canonicalize_and_hash};
pub use record::{IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord};

#[cfg(test)]
mod tests;
