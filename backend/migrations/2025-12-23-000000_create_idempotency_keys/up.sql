-- Create idempotency_keys table for request deduplication.
--
-- This table stores idempotency records for safe request retries. Each record
-- links a client-provided idempotency key to a payload hash and the original
-- response, allowing duplicate requests to be detected and replayed.
--
-- Records are cleaned up periodically based on created_at (default 24h TTL).

CREATE TABLE idempotency_keys (
    -- Client-provided idempotency key (UUID v4).
    key UUID PRIMARY KEY,

    -- SHA-256 hash of the canonicalised request payload (32 bytes).
    payload_hash BYTEA NOT NULL,

    -- Snapshot of the original response to replay on duplicate requests.
    response_snapshot JSONB NOT NULL,

    -- User who made the original request.
    user_id UUID NOT NULL,

    -- When the record was created (used for TTL-based cleanup).
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for TTL-based cleanup queries.
CREATE INDEX idx_idempotency_keys_created_at ON idempotency_keys (created_at);

-- Comment explaining the table's purpose.
COMMENT ON TABLE idempotency_keys IS
    'Stores idempotency records for safe request retries on POST /api/v1/routes';
