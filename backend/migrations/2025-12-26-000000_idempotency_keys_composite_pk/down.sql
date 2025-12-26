-- Revert to single-column primary key on key.
--
-- Fails early if duplicate keys exist across users to avoid leaving the
-- table without any primary key constraint.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM idempotency_keys
        GROUP BY key HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'Cannot revert: duplicate idempotency keys exist across users. Clean up duplicates first.';
    END IF;
END $$;

ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key);
