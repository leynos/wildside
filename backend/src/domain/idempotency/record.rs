//! Stored idempotency records and lookup types.

use chrono::{DateTime, Utc};

use super::super::UserId;
use super::{IdempotencyKey, MutationType, PayloadHash};

/// Stored idempotency record linking a key to its payload and response.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    /// The idempotency key provided by the client.
    pub key: IdempotencyKey,
    /// The type of mutation this record protects.
    pub mutation_type: MutationType,
    /// SHA-256 hash of the canonicalized request payload.
    pub payload_hash: PayloadHash,
    /// Snapshot of the original response to replay.
    pub response_snapshot: serde_json::Value,
    /// User who made the original request.
    pub user_id: UserId,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
}

/// Result of looking up an idempotency key in the store.
#[derive(Debug, Clone)]
pub enum IdempotencyLookupResult {
    /// No record exists for this key.
    NotFound,
    /// A record exists and the payload hash matches.
    MatchingPayload(IdempotencyRecord),
    /// A record exists but the payload hash differs (conflict).
    ConflictingPayload(IdempotencyRecord),
}

/// Query parameters for looking up an idempotency key.
///
/// Bundles the parameters needed for an idempotency lookup into a single struct,
/// reducing the number of arguments passed to repository methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyLookupQuery {
    /// The idempotency key to look up.
    pub key: IdempotencyKey,
    /// The user who made the request.
    pub user_id: UserId,
    /// The type of mutation being performed.
    pub mutation_type: MutationType,
    /// The hash of the request payload.
    pub payload_hash: PayloadHash,
}

impl IdempotencyLookupQuery {
    /// Create a new lookup query.
    pub fn new(
        key: IdempotencyKey,
        user_id: UserId,
        mutation_type: MutationType,
        payload_hash: PayloadHash,
    ) -> Self {
        Self {
            key,
            user_id,
            mutation_type,
            payload_hash,
        }
    }
}
