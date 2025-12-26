-- Revert create_idempotency_keys migration.

DROP TABLE IF EXISTS idempotency_keys;
