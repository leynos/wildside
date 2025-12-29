-- Revert mutation_type addition.
--
-- Includes automated pre-flight duplicate detection: if any (key, user_id) pairs
-- exist across multiple mutation types, the migration aborts with a diagnostic
-- query to identify the affected records.
DO $$
DECLARE
    dup_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO dup_count
    FROM (
        SELECT key, user_id
        FROM idempotency_keys
        GROUP BY key, user_id
        HAVING COUNT(*) > 1
    ) AS duplicates;

    IF dup_count > 0 THEN
        RAISE EXCEPTION 'Cannot revert migration: % duplicate (key, user_id) pair(s) exist across different mutation types. Run this query to identify them: SELECT key, user_id, array_agg(mutation_type) AS mutation_types, COUNT(*) AS cnt FROM idempotency_keys GROUP BY key, user_id HAVING COUNT(*) > 1;', dup_count;
    END IF;
END $$;

-- Drop the index on user_id and mutation_type.
DROP INDEX IF EXISTS idx_idempotency_keys_user_mutation;

-- Drop the composite primary key (key, user_id, mutation_type).
ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;

-- Recreate the original primary key (key, user_id).
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key, user_id);

-- Drop the CHECK constraint.
ALTER TABLE idempotency_keys DROP CONSTRAINT IF EXISTS chk_mutation_type;

-- Drop the mutation_type column.
ALTER TABLE idempotency_keys DROP COLUMN IF EXISTS mutation_type;

-- Restore original table comment.
COMMENT ON TABLE idempotency_keys IS
    'Stores idempotency records for safe request retries on POST /api/v1/routes';
