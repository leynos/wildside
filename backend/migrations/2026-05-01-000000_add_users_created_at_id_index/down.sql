-- Revert users keyset pagination index.

DROP INDEX IF EXISTS idx_users_created_at_id;
