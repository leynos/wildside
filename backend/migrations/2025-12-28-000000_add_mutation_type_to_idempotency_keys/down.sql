-- Revert mutation_type addition.
--
-- WARNING: This migration may fail if there are records with the same (key, user_id)
-- across different mutation types. Such records would need to be manually resolved
-- before reverting.

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
