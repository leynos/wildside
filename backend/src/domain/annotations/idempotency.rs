//! Idempotency helpers for the route annotations service.
//!
//! These types encapsulate payload hashing and idempotency metadata so the
//! service can focus on orchestration.

use serde_json::json;

use crate::domain::ports::{DeleteNoteRequest, UpdateProgressRequest, UpsertNoteRequest};
use crate::domain::{
    IdempotencyKey, IdempotencyLookupQuery, IdempotencyRecord, MutationType, PayloadHash, UserId,
    canonicalize_and_hash,
};

pub(super) struct IdempotentMutationParams<'a, Req> {
    pub(super) request: &'a Req,
    pub(super) user_id: &'a UserId,
    pub(super) mutation_type: MutationType,
    pub(super) idempotency_key: Option<IdempotencyKey>,
}

pub(super) trait PayloadHashable {
    fn compute_payload_hash(&self) -> PayloadHash;
}

pub(super) trait IdempotentMutationRequest: PayloadHashable {
    fn user_id(&self) -> &UserId;
    fn idempotency_key(&self) -> Option<IdempotencyKey>;
    fn mutation_type(&self) -> MutationType;
}

impl PayloadHashable for UpsertNoteRequest {
    fn compute_payload_hash(&self) -> PayloadHash {
        canonicalize_and_hash(&json!({
            "routeId": self.route_id,
            "noteId": self.note_id,
            "poiId": self.poi_id,
            "body": self.body,
            "expectedRevision": self.expected_revision,
        }))
    }
}

impl PayloadHashable for UpdateProgressRequest {
    fn compute_payload_hash(&self) -> PayloadHash {
        canonicalize_and_hash(&json!({
            "routeId": self.route_id,
            "visitedStopIds": self.visited_stop_ids,
            "expectedRevision": self.expected_revision,
        }))
    }
}

impl PayloadHashable for DeleteNoteRequest {
    fn compute_payload_hash(&self) -> PayloadHash {
        canonicalize_and_hash(&json!({
            "noteId": self.note_id,
        }))
    }
}

impl IdempotentMutationRequest for UpsertNoteRequest {
    fn user_id(&self) -> &UserId {
        &self.user_id
    }

    fn idempotency_key(&self) -> Option<IdempotencyKey> {
        self.idempotency_key.clone()
    }

    fn mutation_type(&self) -> MutationType {
        MutationType::Notes
    }
}

impl IdempotentMutationRequest for UpdateProgressRequest {
    fn user_id(&self) -> &UserId {
        &self.user_id
    }

    fn idempotency_key(&self) -> Option<IdempotencyKey> {
        self.idempotency_key.clone()
    }

    fn mutation_type(&self) -> MutationType {
        MutationType::Progress
    }
}

impl IdempotentMutationRequest for DeleteNoteRequest {
    fn user_id(&self) -> &UserId {
        &self.user_id
    }

    fn idempotency_key(&self) -> Option<IdempotencyKey> {
        self.idempotency_key.clone()
    }

    fn mutation_type(&self) -> MutationType {
        MutationType::Notes
    }
}

#[derive(Debug, Clone)]
pub(super) struct IdempotencyContext {
    key: IdempotencyKey,
    mutation_type: MutationType,
    payload_hash: PayloadHash,
    user_id: UserId,
}

impl IdempotencyContext {
    pub(super) fn new(
        key: IdempotencyKey,
        user_id: UserId,
        mutation_type: MutationType,
        payload_hash: PayloadHash,
    ) -> Self {
        Self {
            key,
            mutation_type,
            payload_hash,
            user_id,
        }
    }

    pub(super) fn lookup_query(&self) -> IdempotencyLookupQuery {
        IdempotencyLookupQuery::new(
            self.key.clone(),
            self.user_id.clone(),
            self.mutation_type,
            self.payload_hash.clone(),
        )
    }

    pub(super) fn record(&self, response_snapshot: serde_json::Value) -> IdempotencyRecord {
        IdempotencyRecord {
            key: self.key.clone(),
            mutation_type: self.mutation_type,
            payload_hash: self.payload_hash.clone(),
            response_snapshot,
            user_id: self.user_id.clone(),
            created_at: chrono::Utc::now(),
        }
    }
}
