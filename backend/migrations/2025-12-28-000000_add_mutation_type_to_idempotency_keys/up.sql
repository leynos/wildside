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
--
-- IMPORTANT: Keep this list synchronised with MutationType::ALL in
-- backend/src/domain/idempotency/mod.rs. The test
-- `mutation_type_values_match_migration_constraint` validates this at build time.
ALTER TABLE idempotency_keys
ADD CONSTRAINT chk_mutation_type CHECK (
    mutation_type IN ('routes', 'notes', 'progress', 'preferences', 'bundles')
);

-- Drop the existing primary key (key, user_id) and recreate with mutation_type.
--
-- WARNING: This operation acquires an ACCESS EXCLUSIVE lock on the table,
-- blocking all reads and writes until completion. For large tables, consider:
-- - Running during a maintenance window
-- - Using pg_repack or similar tools for zero-downtime migrations
-- - The idempotency_keys table is expected to be small (records expire via TTL)
ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key, user_id, mutation_type);

-- Create index for lookups by user and mutation type.
CREATE INDEX idx_idempotency_keys_user_mutation
ON idempotency_keys (user_id, mutation_type);

-- Update table comment to reflect multi-mutation support.
COMMENT ON TABLE idempotency_keys IS
    'Stores idempotency records for safe request retries on outbox-backed mutations';
