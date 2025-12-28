-- Add mutation_type column to idempotency_keys table.
--
-- This enables the same idempotency key UUID to be used across different
-- mutation types (routes, notes, progress, preferences, bundles) without
-- collision. The mutation_type becomes part of the composite primary key.

-- Add mutation_type column with NOT NULL and default for existing records.
-- Existing records default to 'routes' for backward compatibility.
ALTER TABLE idempotency_keys
ADD COLUMN mutation_type TEXT NOT NULL DEFAULT 'routes';

-- Add CHECK constraint for known mutation types.
ALTER TABLE idempotency_keys
ADD CONSTRAINT chk_mutation_type CHECK (
    mutation_type IN ('routes', 'notes', 'progress', 'preferences', 'bundles')
);

-- Drop the existing primary key (key, user_id) and recreate with mutation_type.
ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key, user_id, mutation_type);

-- Create index for lookups by user and mutation type (supports TTL cleanup).
CREATE INDEX idx_idempotency_keys_user_mutation
ON idempotency_keys (user_id, mutation_type);

-- Update table comment to reflect multi-mutation support.
COMMENT ON TABLE idempotency_keys IS
    'Stores idempotency records for safe request retries on outbox-backed mutations';
