-- Support keyset pagination over users ordered by creation time.

CREATE INDEX idx_users_created_at_id ON users (created_at, id);
