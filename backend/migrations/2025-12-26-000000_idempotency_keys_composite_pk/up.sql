-- Change idempotency_keys primary key from (key) to (key, user_id).
--
-- This allows different users to independently use the same idempotency key
-- value without conflict, preventing cross-user key reuse attacks.

-- Drop the existing primary key constraint on key alone.
ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;

-- Add composite primary key on (key, user_id).
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key, user_id);
