-- Create example_data_runs table for tracking applied demo data seeds.
-- Used by the example-data feature to ensure once-only seeding.

CREATE TABLE example_data_runs (
    seed_key TEXT PRIMARY KEY,
    seeded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    user_count INTEGER NOT NULL,
    seed BIGINT NOT NULL
);

COMMENT ON TABLE example_data_runs IS
    'Tracks applied example data seeds to prevent duplicate seeding';
