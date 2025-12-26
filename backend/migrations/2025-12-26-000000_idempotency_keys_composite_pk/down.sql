-- Revert to single-column primary key on key.
--
-- Note: This may fail if duplicate keys exist across users.

ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key);
